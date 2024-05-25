use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::{AnyRCDesc, RCDesc};
use crate::rc_slots::{Lock as RCLock, RCSlot, RCSlots, SlotIndex};
use crate::sync::Arc;
use parking_lot::Mutex;
use rangemap::RangeSet;
use std::mem;
use std::mem::ManuallyDrop;
use std::ops::Deref;

pub struct ResourceTable<T: DescTable> {
	slots: Arc<RCSlots<T::Slot, T::RCSlotsInterface>>,
	flush_queue: Mutex<RangeSet<u32>>,
}

impl<T: DescTable> ResourceTable<T> {
	pub fn new(count: u32, interface: T::RCSlotsInterface) -> Self {
		Self {
			slots: RCSlots::new_with_interface(count as usize, interface),
			flush_queue: Mutex::new(RangeSet::new()),
		}
	}

	pub fn alloc_slot<D: DescTypeCpu<DescTable = T>>(&self, cpu_type: <D::DescTable as DescTable>::Slot) -> RCDesc<D> {
		let slot = self.slots.allocate(cpu_type);
		// Safety: we'll pull from the queue later and destroy the slots
		let id = unsafe { slot.clone().into_raw_index().0 } as u32;
		self.flush_queue.lock().insert(id..id + 1);
		RCDesc::<D>::new(slot)
	}

	pub fn try_get_rc(&self, id: u32) -> Option<AnyRCDesc<T>> {
		self.slots
			.try_get_alive_slot(SlotIndex(id as usize))
			.map(AnyRCDesc::new)
	}
}

impl<T: DescTable> Drop for ResourceTable<T> {
	fn drop(&mut self) {
		// ensure all RCSlot's are dropped that are stuck in the flush_queue
		// does not need to be efficient, is only invoked on engine shutdown or panic unwind
		drop(self.flush_updates());
	}
}

// lock
impl<T: DescTable> ResourceTable<T> {
	pub fn lock(&self) -> Lock<T> {
		Lock(self.slots.lock())
	}
}

pub struct Lock<T: DescTable>(RCLock<T::Slot, T::RCSlotsInterface>);

impl<T: DescTable> Deref for Lock<T> {
	type Target = RCLock<T::Slot, T::RCSlotsInterface>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

// flush_updates
impl<T: DescTable> ResourceTable<T> {
	/// Flushes all queued up updates. The `f` function is called with the `first_array_index` and a `&mut Vec` of
	/// `SlotType`s, that should be [`Vec::drain`]-ed by the function, leaving the Vec empty.
	pub(crate) fn flush_updates(&self) -> FlushUpdates<T> {
		let mut ranges = self.flush_queue.lock();
		let ranges = if ranges.is_empty() {
			RangeSet::new()
		} else {
			mem::replace(&mut *ranges, RangeSet::new())
		};
		FlushUpdates { table: self, ranges }
	}
}

pub struct FlushUpdates<'a, T: DescTable> {
	table: &'a ResourceTable<T>,
	ranges: RangeSet<u32>,
}

/// this type only exists as you cannot nest `impl Trait`
pub struct FlushSequence<'a, T: DescTable>(
	&'a mut Vec<ManuallyDrop<RCSlot<<T as DescTable>::Slot, <T as DescTable>::RCSlotsInterface>>>,
);

impl<'a, T: DescTable> FlushSequence<'a, T> {
	pub fn iter(&mut self) -> impl Iterator<Item = &T::Slot> {
		self.0.iter().map(|slot| &***slot)
	}

	pub fn capacity(&self) -> usize {
		self.0.capacity()
	}
}

impl<'a, T: DescTable> FlushUpdates<'a, T> {
	pub fn iter(&self, mut f: impl FnMut(u32, &mut FlushSequence<T>)) {
		if self.ranges.is_empty() {
			return;
		}

		// allocate for worst possible case right away
		let max = self.ranges.iter().map(|r| r.end - r.start).max().unwrap();
		let mut buffer = Vec::with_capacity(max as usize);

		for range in self.ranges.iter() {
			let range = (range.start as usize)..(range.end as usize);
			for index in range.clone() {
				// don't drop the here, drop them when FlushUpdates is dropped
				// Safety: indices come from alloc_slot
				buffer.push(unsafe { ManuallyDrop::new(RCSlot::from_raw_index(&self.table.slots, SlotIndex(index))) });
			}

			f(range.start as u32, &mut FlushSequence(&mut buffer));
			buffer.clear();
		}
	}
}

impl<'a, T: DescTable> Drop for FlushUpdates<'a, T> {
	fn drop(&mut self) {
		for range in self.ranges.iter() {
			let range = (range.start as usize)..(range.end as usize);
			for index in range.clone() {
				// Safety: indices come from alloc_slot
				drop(unsafe { RCSlot::from_raw_index(&self.table.slots, SlotIndex(index)) });
			}
		}
	}
}
