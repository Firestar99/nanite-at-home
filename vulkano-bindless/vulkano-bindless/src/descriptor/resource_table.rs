use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::rc_slots::RCSlots;
use crate::sync::Arc;
use parking_lot::Mutex;
use rangemap::RangeSet;
use smallvec::SmallVec;
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
		let desc = RCDesc::<D>::new(self.slots.allocate(D::to_table(cpu_type)));
		let id = desc.id();
		self.flush_queue.lock().insert(id..id + 1);
		desc
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

		let cleanup_lock = self.slots.cleanup_lock();
		let mut iter = cleanup_lock.iter_latest_with(|slot| slot.map(|s| s.with(|s| s.clone())));
		let mut iter_index = 0;
		for range in ranges.iter() {
			let mut write_start = range.start;
			let mut push = |s: Option<T::SlotType>| {
				if let Some(slot) = s {
					buffer.push(slot)
				} else {
					let len = buffer.len();
					if len != 0 {
						writes.push(T::write_descriptor_set(T::BINDING, write_start, buffer.drain(..)))
					}
					write_start += len as u32 + 1;
				}
			};

			let range = (range.start as usize)..(range.end as usize);
			// advance would be better here, but that's still feature flagged in nightly
			push(iter.nth(range.start - iter_index).unwrap());
			for _ in 0..(range.end - range.start - 1) {
				push(iter.next().unwrap());
			}
			iter_index = range.end;

			// flush
			push(None);
		}

		ranges.clear();
	}
}
