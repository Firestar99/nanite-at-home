use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::rc_slots::{RCSlot, RCSlots, SlotIndex};
use crate::sync::Arc;
use parking_lot::Mutex;
use rangemap::RangeSet;
use smallvec::SmallVec;
use std::ops::Deref;
use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayoutBinding};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::Device;
use vulkano::shader::ShaderStages;

pub struct ResourceTable<T: ResourceTableCpu> {
	slots: Arc<RCSlots<T::SlotType>>,
	flush_queue: Mutex<RangeSet<u32>>,
}

impl<T: ResourceTableCpu> ResourceTable<T> {
	pub fn new(count: u32) -> Self {
		Self {
			slots: RCSlots::new(count as usize),
			flush_queue: Mutex::new(RangeSet::new()),
		}
	}

	pub fn layout_binding(device: &Arc<Device>, stages: ShaderStages, count: u32) -> (u32, DescriptorSetLayoutBinding) {
		let max = T::max_update_after_bind_descriptors(device.physical_device());
		assert!(
			count <= max,
			"Requested descriptors {} exceeds max descriptor count {}!",
			count,
			max
		);
		(
			T::BINDING,
			DescriptorSetLayoutBinding {
				binding_flags: DescriptorBindingFlags::UPDATE_AFTER_BIND
					| DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING
					| DescriptorBindingFlags::PARTIALLY_BOUND,
				descriptor_count: count,
				stages,
				..DescriptorSetLayoutBinding::descriptor_type(T::DESCRIPTOR_TYPE)
			},
		)
	}

	pub fn alloc_slot<D: DescTypeCpu<ResourceTableCpu = T>>(&self, cpu_type: D::CpuType) -> RCDesc<D> {
		let slot = self.slots.allocate(D::to_table(cpu_type));
		// Safety: we'll pull from the queue later and destroy the slots
		let id = unsafe { slot.clone().into_raw_index().0 } as u32;
		self.flush_queue.lock().insert(id..id + 1);
		RCDesc::<D>::new(slot)
	}

	pub(crate) fn flush_updates<const C: usize>(&self, writes: &mut SmallVec<[WriteDescriptorSet; C]>) {
		let mut ranges = self.flush_queue.lock();
		if ranges.is_empty() {
			return;
		}

		// allocate for worst possible case right away
		writes.reserve(ranges.iter().count());
		let max = ranges.iter().map(|r| r.end - r.start).max().unwrap();
		let mut buffer = Vec::with_capacity(max as usize);

		for range in ranges.iter() {
			let range = (range.start as usize)..(range.end as usize);
			for index in range.clone() {
				// Safety: indices come from alloc_slot
				let slot = unsafe { RCSlot::from_raw_index(&self.slots, SlotIndex(index)) };
				buffer.push(slot.deref().clone());
				// may want to delay dropping?
				drop(slot);
			}
			writes.push(T::write_descriptor_set(
				T::BINDING,
				range.start as u32,
				buffer.drain(..),
			));
		}

		ranges.clear();
	}
}
