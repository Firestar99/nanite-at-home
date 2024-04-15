use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::rc_slots::{AtomicRCSlots, AtomicRCSlotsLock};
use crate::sync::Arc;
use parking_lot::Mutex;
use rangemap::RangeSet;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use vulkano::descriptor_set::allocator::DescriptorSetAllocator;
use vulkano::descriptor_set::layout::{
	DescriptorBindingFlags, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
	DescriptorSetLayoutCreateInfo,
};
use vulkano::descriptor_set::DescriptorSet;
use vulkano::device::Device;
use vulkano::shader::ShaderStages;

pub struct ResourceTable<T: ResourceTableCpu> {
	pub device: Arc<Device>,
	pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	pub descriptor: Arc<DescriptorSet>,
	slots: Arc<AtomicRCSlots<T::SlotType>>,
	flush_queue: Mutex<RangeSet<u32>>,
}

impl<T: ResourceTableCpu> ResourceTable<T> {
	pub fn new(
		device: Arc<Device>,
		stages: ShaderStages,
		allocator: Arc<dyn DescriptorSetAllocator>,
		count: u32,
	) -> Self {
		let descriptor_set_layout = DescriptorSetLayout::new(
			device.clone(),
			DescriptorSetLayoutCreateInfo {
				flags: DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
				bindings: BTreeMap::from([(
					0,
					DescriptorSetLayoutBinding {
						binding_flags: DescriptorBindingFlags::UPDATE_AFTER_BIND
							| DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING
							| DescriptorBindingFlags::PARTIALLY_BOUND
							| DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT,
						descriptor_count: T::max_update_after_bind_descriptors(device.physical_device()),
						stages,
						..DescriptorSetLayoutBinding::descriptor_type(T::DESCRIPTOR_TYPE)
					},
				)]),
				..DescriptorSetLayoutCreateInfo::default()
			},
		)
		.unwrap();
		let descriptor = DescriptorSet::new_variable(allocator, descriptor_set_layout.clone(), count, [], []).unwrap();

		Self {
			device,
			descriptor_set_layout,
			descriptor,
			slots: AtomicRCSlots::new(count as usize),
			flush_queue: Mutex::new(RangeSet::new()),
		}
	}

	pub fn alloc_slot<D: DescTypeCpu<ResourceTableCpu = T>>(&self, cpu_type: D::CpuType) -> RCDesc<D> {
		let desc = RCDesc::<D>::new(self.slots.allocate(D::to_table(cpu_type)));
		let id = desc.id();
		self.flush_queue.lock().insert(id..id + 1);
		desc
	}

	// FIXME: this lock has a certain view on our slots, and may hide certain entries which are in the reaper queue. However, we must access all entries, whether in the reaper queue or not.
	pub fn flush(&self, lock: AtomicRCSlotsLock<T::SlotType>) {
		let mut ranges = self.flush_queue.lock();
		if ranges.is_empty() {
			return;
		}

		// allocate for worst possible case right away
		let max = ranges.iter().map(|r| r.end - r.start).max().unwrap();
		let mut buffer = Vec::with_capacity(max as usize);

		let mut iter = lock.iter_with(|slot| slot.map(|s| s.with(|s| s.clone())));
		let mut iter_index = 0;
		let mut writes: SmallVec<[_; 8]> = SmallVec::new();
		for range in ranges.iter() {
			let mut write_start = range.start;
			let mut push = |s: Option<T::SlotType>| {
				if let Some(slot) = s {
					buffer.push(slot)
				} else {
					let len = buffer.len();
					if len != 0 {
						writes.push(T::write_descriptor_set(0, write_start, buffer.drain(..)))
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

		// Safety: update-after-bind descriptors have relaxed external synchronization requirements:
		//	* only one thread may update at once, ensured by flush_queue Mutex
		//  * descriptor set may be used in command buffers concurrently, see spec
		unsafe {
			self.descriptor.update_by_ref(writes, []).unwrap();
		}

		ranges.clear();
	}
}
