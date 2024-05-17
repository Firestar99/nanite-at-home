use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::rc_slots::{Lock, RCSlot, RCSlots, SlotIndex};
use crate::sync::Arc;
use parking_lot::Mutex;
use rangemap::RangeSet;
use std::ops::Deref;

pub struct ResourceTable<T: DescTable> {
	slots: Arc<RCSlots<T::Slot>>,
	flush_queue: Mutex<RangeSet<u32>>,
}

impl<T: DescTable> ResourceTable<T> {
	pub fn new(count: u32) -> Self {
		Self {
			slots: RCSlots::new(count as usize),
			flush_queue: Mutex::new(RangeSet::new()),
		}
	}

	pub fn alloc_slot<D: DescTypeCpu<DescTable = T>>(&self, cpu_type: D::VulkanType) -> RCDesc<D> {
		let slot = self.slots.allocate(D::to_table(cpu_type));
		// Safety: we'll pull from the queue later and destroy the slots
		let id = unsafe { slot.clone().into_raw_index().0 } as u32;
		self.flush_queue.lock().insert(id..id + 1);
		RCDesc::<D>::new(slot)
	}

	/// Flushes all queued up updates. The `f` function is called with the `first_array_index` and a `&mut Vec` of
	/// `SlotType`s, that should be [`Vec::drain`]-ed by the function, leaving the Vec empty.
	pub(crate) fn flush_updates(&self, mut f: impl FnMut(u32, &mut Vec<<T as DescTable>::Slot>)) {
		let mut ranges = self.flush_queue.lock();
		if ranges.is_empty() {
			return;
		}

		// allocate for worst possible case right away
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

			f(range.start as u32, &mut buffer);
			assert!(buffer.is_empty());
		}

		ranges.clear();
	}

	pub fn lock(&self) -> Lock<T::Slot> {
		self.slots.lock()
	}
}

impl<T: DescTable> Drop for ResourceTable<T> {
	fn drop(&mut self) {
		// ensure all RCSlot's are dropped what are stuck in the flush_queue
		// does not need to be efficient, is only invoked on engine shutdown or panic unwind
		self.flush_updates(|_, vec| vec.clear())
	}
}
