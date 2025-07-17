use crate::backing::ab::{AB, ABArray};
use crate::backing::range_set::{DescriptorIndexIterator, DescriptorIndexRangeSet};
use crate::backing::slot_array::SlotArray;
use crossbeam_queue::SegQueue;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, MutexGuard, RwLock};
use rust_gpu_bindless_shaders::descriptor::{
	DescriptorId, DescriptorIndex, DescriptorType, DescriptorVersion, ID_TYPE_BITS,
};
use static_assertions::const_assert_eq;
use std::any::Any;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{Deref, Index};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicU32, fence};
use std::sync::{Arc, Weak};

pub trait TableInterface: Sized + 'static {
	type Slot;
	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>);
	fn flush<'a>(&self, flush_queue: impl DescriptorIndexIterator<'a, Self>);
}

pub const TABLE_COUNT: u32 = 1 << ID_TYPE_BITS;

#[repr(C)]
pub struct TableSync {
	// TODO I hate this RwLock
	tables: [RwLock<Option<Arc<dyn AbstractTable>>>; TABLE_COUNT as usize],
	frame_mutex: CachePadded<Mutex<ABArray<u32>>>,
	table_next_free: CachePadded<AtomicU32>,
	write_queue_ab: CachePadded<AtomicU32>,
	/// Mutex for both flushing ang gc. Ensures multiple flushes do not race and prevents gc-ing while flushing.
	flush_and_gc_mutex: CachePadded<Mutex<()>>,
}

unsafe impl Send for TableSync {}
unsafe impl Sync for TableSync {}

#[repr(C)]
pub struct Table<I: TableInterface> {
	table_sync: Weak<TableSync>,
	table_id: DescriptorType,
	slot_counters: SlotArray<SlotCounter>,
	slots: SlotArray<UnsafeCell<MaybeUninit<I::Slot>>>,
	flush_queue: SegQueue<RcTableSlot>,
	reaper_queue: ABArray<SegQueue<DescriptorIndex>>,
	dead_queue: SegQueue<DescriptorIndex>,
	next_free: CachePadded<AtomicU32>,
	interface: I,
}

unsafe impl<I: TableInterface> Send for Table<I> {}
unsafe impl<I: TableInterface> Sync for Table<I> {}

impl TableSync {
	pub fn new() -> Arc<Self> {
		Arc::new(TableSync {
			tables: core::array::from_fn(|_| RwLock::new(None)),
			table_next_free: CachePadded::new(AtomicU32::new(0)),
			frame_mutex: CachePadded::new(Mutex::new(ABArray::new(|| 0))),
			write_queue_ab: CachePadded::new(AtomicU32::new(AB::B.to_u32())),
			flush_and_gc_mutex: CachePadded::new(Mutex::new(())),
		})
	}

	pub fn register<I: TableInterface>(
		self: &Arc<Self>,
		slots_capacity: u32,
		interface: I,
	) -> Result<Arc<Table<I>>, TableRegisterError> {
		let table_id = self.table_next_free.fetch_add(1, Relaxed);
		if table_id < TABLE_COUNT {
			let mut guard = self.tables[table_id as usize].write();
			let table = Arc::new(Table {
				table_sync: Arc::downgrade(self),
				table_id: unsafe { DescriptorType::new(table_id).unwrap() },
				interface,
				slot_counters: SlotArray::new(slots_capacity),
				slots: SlotArray::new_generator(slots_capacity, |_| UnsafeCell::new(MaybeUninit::uninit())),
				flush_queue: SegQueue::new(),
				reaper_queue: ABArray::new(SegQueue::new),
				dead_queue: SegQueue::new(),
				next_free: CachePadded::new(AtomicU32::new(0)),
			});
			let old_table = guard.replace(table.clone() as Arc<dyn AbstractTable>);
			assert!(old_table.is_none());
			Ok(table)
		} else {
			Err(TableRegisterError::OutOfTables)
		}
	}

	#[inline]
	fn write_queue_ab(&self) -> AB {
		AB::from_u32(self.write_queue_ab.load(Relaxed)).unwrap()
	}

	#[inline]
	fn frame_ab(&self) -> AB {
		!self.write_queue_ab()
	}

	pub fn frame(self: &Arc<Self>) -> FrameGuard {
		let frame_ab;
		{
			let mut guard = self.frame_mutex.lock();
			frame_ab = self.frame_ab();
			guard[frame_ab] += 1;

			// if we ran dry of frames (like we are at startup), switch frame ab after first frame
			if guard[!frame_ab] == 0 {
				self.gc_queue(guard, !frame_ab);
			}
		}

		FrameGuard {
			table_manager: self.clone(),
			frame_ab,
		}
	}

	fn frame_drop(self: &Arc<Self>, dropped_frame_ab: AB) {
		let mut guard = self.frame_mutex.lock();
		let frame_cnt = &mut guard[dropped_frame_ab];
		match *frame_cnt {
			0 => panic!("frame ref counting underflow"),
			1 => {
				*frame_cnt = 0;
				let frame_ab = self.frame_ab();
				if frame_ab != dropped_frame_ab {
					self.gc_queue(guard, dropped_frame_ab);
				}
			}
			_ => *frame_cnt -= 1,
		}
	}

	#[cold]
	#[inline(never)]
	fn gc_queue(&self, guard: MutexGuard<ABArray<u32>>, dropped_frame_ab: AB) {
		let _guard2 = self.flush_and_gc_mutex.lock();
		let table_gc_indices;
		{
			let gc_queue = !dropped_frame_ab;
			table_gc_indices = self
				.tables
				.iter()
				.map(|table_lock| {
					let table = table_lock.read();
					table.as_ref().map(|table| table.gc_collect(gc_queue))
				})
				.collect::<Vec<_>>();

			// Release may seem a bit defensive here, as we don't actually need to flush any memory.
			// But it ensures that when creating a new FrameGuard afterward and sending it to another thread via
			// Rel/Acq, this write is visible. Which is important as it could otherwise write to be gc'ed objects to the
			// wrong queue.
			self.write_queue_ab.store(gc_queue.to_u32(), Release);
			drop(guard);
		}

		for (table, gc_indices) in self.tables.iter().zip(table_gc_indices) {
			if let Some(gc_indices) = gc_indices {
				if !gc_indices.is_empty() {
					let table = table.read();
					if let Some(table) = table.as_ref() {
						table.gc_drop(gc_indices)
					} else {
						unreachable!()
					}
				}
			}
		}
	}

	pub fn try_recover(self: &Arc<Self>, id: DescriptorId) -> Option<RcTableSlot> {
		let table = self.tables[id.desc_type().to_usize()].read();
		if let Some(table) = table.as_ref() {
			table
				.try_recover(id, self.write_queue_ab())
				.then(|| unsafe { RcTableSlot::new(Arc::as_ptr(self), id) })
		} else {
			None
		}
	}

	/// Flush all tables
	pub fn flush(&self) {
		self.flush_lock().flush();
	}

	/// Acquire the [`FlushGuard`] that may be used to flush tables and allows building manual flushing algorithms.
	pub fn flush_lock(&self) -> FlushGuard<'_> {
		FlushGuard {
			table_sync: self,
			_guard: self.flush_and_gc_mutex.lock(),
		}
	}
}

pub struct FlushGuard<'a> {
	table_sync: &'a TableSync,
	_guard: MutexGuard<'a, ()>,
}

impl FlushGuard<'_> {
	pub fn flush(&self) {
		for table_lock in &self.table_sync.tables {
			if let Some(table) = table_lock.read().as_ref() {
				table.flush();
			}
		}
	}
}

impl<I: TableInterface> Table<I> {
	#[inline]
	pub fn slots_capacity(&self) -> u32 {
		self.slot_counters.len() as u32
	}

	pub fn alloc_slot(self: &Arc<Self>, slot: I::Slot) -> Result<RcTableSlot, SlotAllocationError> {
		let index = if let Some(index) = self.dead_queue.pop() {
			Ok(index)
		} else {
			let index = self.next_free.fetch_add(1, Relaxed);
			if index < self.slots_capacity() {
				// Safety: atomic ensures it's unique
				unsafe { Ok(DescriptorIndex::new(index).unwrap()) }
			} else {
				Err(SlotAllocationError::NoMoreCapacity(self.slots_capacity()))
			}
		}?;

		// Safety: we just allocated index, we have exclusive access to slot, which is currently uninitialized
		unsafe { (*self.slots[index].get()).write(slot) };
		let slot = &self.slot_counters[index];
		slot.ref_count.store(2, Release);

		// Safety: this is a valid id, we transfer the **2** ref_count inc above to the **2** RcTableSlots created
		unsafe {
			let id = DescriptorId::new(self.table_id, index, slot.read_version());
			let table = Arc::into_raw(self.table_sync.upgrade().expect("alloc_slot during destruction"));
			self.flush_queue.push(RcTableSlot::new(table, id));
			Ok(RcTableSlot::new(table, id))
		}
	}

	/// Get the contents of the slot unchecked
	///
	/// # Safety
	/// Assumes the slot is initialized
	pub unsafe fn get_slot_unchecked(&self, index: DescriptorIndex) -> &I::Slot {
		unsafe { (*self.slots[index].get()).assume_init_ref() }
	}

	#[inline]
	pub fn drain_flush_queue(&self) -> DrainFlushQueue<'_, I> {
		DrainFlushQueue(self)
	}
}

impl<I: TableInterface> Deref for Table<I> {
	type Target = I;

	fn deref(&self) -> &Self::Target {
		&self.interface
	}
}

/// Internal Trait
trait AbstractTable: Any + Send + Sync + 'static {
	fn as_any(&self) -> &dyn Any;
	fn ref_inc(&self, id: DescriptorId);
	fn ref_dec(&self, id: DescriptorId, write_queue_ab: AB) -> bool;
	fn gc_collect(&self, gc_queue: AB) -> DescriptorIndexRangeSet<'static, ()>;
	fn gc_drop(&self, gc_indices: DescriptorIndexRangeSet<'static, ()>);
	fn flush(&self);
	fn try_recover(&self, id: DescriptorId, write_queue_ab: AB) -> bool;
}

impl<I: TableInterface> AbstractTable for Table<I> {
	fn as_any(&self) -> &dyn Any {
		self
	}

	#[inline]
	fn ref_inc(&self, id: DescriptorId) {
		self.slot_counters[id.index()].ref_count.fetch_add(1, Relaxed);
	}

	#[inline]
	fn ref_dec(&self, id: DescriptorId, write_queue_ab: AB) -> bool {
		match self.slot_counters[id.index()].ref_count.fetch_sub(1, Relaxed) {
			0 => panic!("TableSlot ref_count underflow!"),
			1 => {
				fence(Acquire);
				self.reaper_queue[write_queue_ab].push(id.index());
				true
			}
			_ => false,
		}
	}

	fn gc_collect(&self, gc_queue: AB) -> DescriptorIndexRangeSet<'static, ()> {
		let reaper_queue = &self.reaper_queue[gc_queue];
		unsafe { DescriptorIndexRangeSet::from(&(), (0..).map_while(|_| reaper_queue.pop())) }
	}

	fn gc_drop(&self, gc_indices: DescriptorIndexRangeSet<'static, ()>) {
		let gc_indices = unsafe { DescriptorIndexRangeSet::new(self, gc_indices.into_range_set()) };

		self.interface.drop_slots(&gc_indices);

		for (i, _) in gc_indices.iter() {
			// Safety: we have exclusive access to the previously initialized slot
			let valid_version = unsafe {
				(*self.slots.index(i).get()).assume_init_drop();
				let version = &mut *self.slot_counters[i].version.get();
				*version += 1;
				DescriptorVersion::new(*version).is_some()
			};

			// we send / share the slot to the dead_queue
			if valid_version {
				self.dead_queue.push(i);
			}
		}
	}

	fn flush(&self) {
		self.interface.flush(&mut self.drain_flush_queue())
	}

	fn try_recover(&self, id: DescriptorId, write_queue_ab: AB) -> bool {
		let counters = &self.slot_counters[id.index()];
		let mut old = counters.ref_count.load(Acquire);
		loop {
			if old == 0 {
				// slot has been deallocated
				break false;
			}

			match counters.ref_count.compare_exchange_weak(old, old + 1, Acquire, Relaxed) {
				Ok(_) => {
					// Safety: we inc ref count so this slot must be alive and we can read this
					let version = unsafe { *counters.version.get() };
					if id.version().to_u32() == version {
						break true;
					} else {
						// slot has been reused
						self.ref_dec(id, write_queue_ab);
						break false;
					}
				}
				Err(o) => old = o,
			}
		}
	}
}

impl<I: TableInterface> Drop for Table<I> {
	fn drop(&mut self) {
		self.flush();
		for ab in AB::VALUES {
			self.gc_drop(self.gc_collect(ab))
		}
	}
}

#[derive(Debug)]
struct SlotCounter {
	ref_count: AtomicU32,
	version: UnsafeCell<u32>,
}
const_assert_eq!(core::mem::size_of::<SlotCounter>(), 8);

impl SlotCounter {
	/// # Safety
	/// creates a reference to `self.version`
	unsafe fn read_version(&self) -> DescriptorVersion {
		unsafe { DescriptorVersion::new(*self.version.get()).unwrap() }
	}
}

impl Default for SlotCounter {
	fn default() -> Self {
		Self {
			ref_count: AtomicU32::new(0),
			version: UnsafeCell::new(0),
		}
	}
}

#[derive(Eq, PartialEq, Hash)]
pub struct RcTableSlot {
	table: *const TableSync,
	id: DescriptorId,
}

unsafe impl Send for RcTableSlot {}
unsafe impl Sync for RcTableSlot {}

impl RcTableSlot {
	/// Creates a mew RcTableSlot
	///
	/// # Safety
	/// This function will take ownership of one `ref_count` increment of the slot.
	#[inline]
	unsafe fn new(table: *const TableSync, id: DescriptorId) -> Self {
		Self { table, id }
	}

	#[inline]
	pub fn table_sync(&self) -> &TableSync {
		unsafe { &*self.table }
	}

	#[inline]
	pub fn table_sync_arc(&self) -> Arc<TableSync> {
		unsafe {
			// don't touch the ref count, even if we panic
			let arc = ManuallyDrop::new(Arc::from_raw(self.table));
			// only after the clone we may access the plain Arc without ManuallyDrop wrapping
			ManuallyDrop::into_inner(arc.clone())
		}
	}

	#[inline]
	fn table<R>(&self, f: impl FnOnce(&Arc<dyn AbstractTable>) -> R) -> R {
		let guard = self.table_sync().tables[self.id.desc_type().to_usize()].read();
		f(guard.as_ref().unwrap())
	}

	#[inline]
	pub fn id(&self) -> DescriptorId {
		self.id
	}

	pub fn try_deref<I: TableInterface>(&self) -> Option<&I::Slot> {
		self.table(|table| {
			table
				.as_any()
				.downcast_ref::<Table<I>>()
				.map(|table| unsafe { (*table.slots.index(self.id.index()).get()).assume_init_ref() })
		})
	}
}

impl Debug for RcTableSlot {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RcTableSlot").field("id", &self.id).finish()
	}
}

impl Clone for RcTableSlot {
	#[inline]
	fn clone(&self) -> Self {
		self.table(|t| t.ref_inc(self.id));
		unsafe { Self::new(self.table, self.id) }
	}
}

impl Drop for RcTableSlot {
	#[inline]
	fn drop(&mut self) {
		let write_queue_ab = self.table_sync().write_queue_ab();
		if self.table(|t| t.ref_dec(self.id, write_queue_ab)) {
			// Safety: slot ref count hit 0, so decrement ref count of `TableManager` which was incremented in
			// `alloc_slot()` when this slot was created
			unsafe { drop(Arc::from_raw(self.table)) };
		}
	}
}

pub struct FrameGuard {
	table_manager: Arc<TableSync>,
	frame_ab: AB,
}

impl FrameGuard {
	pub fn table_manager(&self) -> &Arc<TableSync> {
		&self.table_manager
	}

	pub fn ab(&self) -> AB {
		self.frame_ab
	}
}

impl Drop for FrameGuard {
	fn drop(&mut self) {
		self.table_manager.frame_drop(self.frame_ab);
	}
}

pub struct DrainFlushQueue<'a, I: TableInterface>(&'a Table<I>);

impl<'a, I: TableInterface> DescriptorIndexIterator<'a, I> for &mut DrainFlushQueue<'a, I> {
	fn into_inner(self) -> (&'a Table<I>, impl Iterator<Item = DescriptorIndex>) {
		(self.0, self)
	}
}

impl<I: TableInterface> Iterator for DrainFlushQueue<'_, I> {
	type Item = DescriptorIndex;

	fn next(&mut self) -> Option<Self::Item> {
		let slot = self.0.flush_queue.pop()?;
		Some(slot.id.index())
		// drops slot, but slot can't be gc-ed until flushing finishes, as ensured by `flush_and_gc_mutex`
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.0.flush_queue.len(), None)
	}
}

#[derive(Debug)]
pub enum TableRegisterError {
	OutOfTables,
}

impl Error for TableRegisterError {}

impl Display for TableRegisterError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			TableRegisterError::OutOfTables => write!(
				f,
				"Registration failed due to running out of table ids, current max is {:?}",
				TABLE_COUNT
			),
		}
	}
}

#[derive(Debug)]
pub enum SlotAllocationError {
	NoMoreCapacity(u32),
}

impl Error for SlotAllocationError {}

impl Display for SlotAllocationError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			SlotAllocationError::NoMoreCapacity(cap) => {
				write!(f, "Ran out of available slots with a capacity of {}!", *cap)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backing::ab::AB::*;
	use rangemap::RangeSet;
	use std::mem::take;

	struct DummyInterface;

	impl TableInterface for DummyInterface {
		type Slot = Arc<u32>;

		fn drop_slots<'a>(&self, _indices: impl DescriptorIndexIterator<'a, Self>) {}

		fn flush<'a>(&self, flush_queue: impl DescriptorIndexIterator<'a, Self>) {
			for _ in flush_queue.into_iter() {}
		}
	}

	struct SimpleInterface {
		drops: Mutex<Vec<RangeSet<DescriptorIndex>>>,
	}

	impl SimpleInterface {
		pub fn new() -> Self {
			Self {
				drops: Mutex::new(Vec::new()),
			}
		}

		pub fn take(&self) -> Vec<Vec<u32>> {
			take(&mut *self.drops.lock())
				.into_iter()
				.map(|set| set.iter().flat_map(|i| i.start.to_u32()..i.end.to_u32()).collect())
				.collect()
		}
	}

	impl TableInterface for SimpleInterface {
		type Slot = Arc<u32>;

		fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
			self.drops.lock().push(indices.into_range_set().into_range_set());
		}

		fn flush<'a>(&self, flush_queue: impl DescriptorIndexIterator<'a, Self>) {
			for _ in flush_queue.into_iter() {}
		}
	}

	pub fn simple_empty() -> Vec<Vec<u32>> {
		Vec::<Vec<u32>>::new()
	}

	#[test]
	fn test_table_register() -> anyhow::Result<()> {
		let tm = TableSync::new();
		tm.register(128, DummyInterface)?;
		Ok(())
	}

	#[test]
	fn test_alloc_slot() -> anyhow::Result<()> {
		const N: u32 = 4;

		let tm = TableSync::new();
		let table = tm.register(N, DummyInterface)?;

		{
			let _slots = (0..N)
				.map(|i| {
					let slot = table.alloc_slot(Arc::new(42 + i)).unwrap();
					assert_eq!(slot.id.index().to_u32(), i);
					assert_eq!(slot.id.desc_type().to_u32(), 0);
					assert_eq!(slot.id.version().to_u32(), 0);
					assert_eq!(**slot.try_deref::<DummyInterface>().unwrap(), 42 + i);
					slot
				})
				.collect::<Vec<_>>();

			table.alloc_slot(Arc::new(69)).expect_err("we should be out of slots");
			table
				.alloc_slot(Arc::new(70))
				.expect_err("asking again but still out of slots");
		}
		tm.flush();

		Ok(())
	}

	#[test]
	fn test_slot_reuse() -> anyhow::Result<()> {
		let tm = TableSync::new();
		let table = tm.register(128, DummyInterface)?;

		let alloc = |cnt: u32, exp_offset: u32, exp_version: u32| {
			let vec = (0..cnt)
				.map(|i| {
					let slot = table.alloc_slot(Arc::new(42 + i)).unwrap();
					assert_eq!(**slot.try_deref::<DummyInterface>().unwrap(), 42 + i);
					assert_eq!(slot.id.index().to_u32(), i + exp_offset);
					assert_eq!(slot.id.version().to_u32(), exp_version);
					slot
				})
				.collect::<Vec<_>>();
			tm.flush();
			vec
		};
		let flush = || {
			for _ in 0..3 {
				drop(tm.frame());
			}
		};

		let alloc1 = alloc(5, 0, 0);
		let alloc2 = alloc(8, 5, 0);
		drop(alloc1);
		flush();

		let alloc1 = alloc(5, 0, 1);
		let alloc3 = alloc(3, 5 + 8, 0);
		drop(alloc2);
		flush();

		let alloc2 = alloc(8, 5, 1);
		let alloc4 = alloc(1, 5 + 8 + 3, 0);
		drop((alloc1, alloc2, alloc3));
		flush();

		let alloc1 = alloc(5, 0, 2);
		let alloc2 = alloc(8, 5, 2);
		let alloc3 = alloc(3, 5 + 8, 1);
		let alloc5 = alloc(2, 5 + 8 + 3 + 1, 0);
		drop((alloc1, alloc2, alloc3, alloc4, alloc5));

		Ok(())
	}

	#[test]
	fn test_frames_sequential() -> anyhow::Result<()> {
		let tm = TableSync::new();
		tm.register(128, DummyInterface)?;

		let frame = |exp: AB| {
			let f = tm.frame();
			assert_eq!(f.frame_ab, exp);
			drop(f);
		};

		for _ in 0..5 {
			frame(A);
		}

		Ok(())
	}

	#[test]
	fn test_frames_dry_out() -> anyhow::Result<()> {
		let tm = TableSync::new();
		tm.register(128, DummyInterface)?;

		for i in 0..5 {
			println!("iter {}", i);
			let flip = |ab: AB| if i % 2 == 0 { ab } else { !ab };

			assert_eq!(tm.frame_ab(), flip(A));
			let a1 = tm.frame();
			assert_eq!(a1.frame_ab, flip(A));

			assert_eq!(tm.frame_ab(), flip(B));
			let b1 = tm.frame();
			assert_eq!(b1.frame_ab, flip(B));

			assert_eq!(tm.frame_ab(), flip(B));
			drop(a1);
			assert_eq!(tm.frame_ab(), flip(A));
			drop(b1);
			assert_eq!(tm.frame_ab(), flip(B));
		}
		Ok(())
	}

	#[test]
	fn test_frames_interleaved() -> anyhow::Result<()> {
		let tm = TableSync::new();
		tm.register(128, DummyInterface)?;

		let a1 = tm.frame();
		assert_eq!(a1.frame_ab, A);

		let b1 = tm.frame();
		assert_eq!(b1.frame_ab, B);
		let b2 = tm.frame();
		assert_eq!(b2.frame_ab, B);

		drop(a1);
		let a2 = tm.frame();
		assert_eq!(a2.frame_ab, A);
		let a3 = tm.frame();
		assert_eq!(a3.frame_ab, A);

		drop((b1, b2));
		let b3 = tm.frame();
		assert_eq!(b3.frame_ab, B);

		// no switch!
		drop(b3);
		let b4 = tm.frame();
		assert_eq!(b4.frame_ab, B);

		Ok(())
	}

	struct FrameSwitch {
		tm: Arc<TableSync>,
		frame: ABArray<Option<FrameGuard>>,
		ab: AB,
	}

	impl FrameSwitch {
		pub fn new(tm: Arc<TableSync>) -> Self {
			let mut switch = Self {
				tm,
				frame: ABArray::new(|| None),
				ab: A,
			};
			for _ in 0..3 {
				switch.switch();
			}
			switch
		}

		pub fn switch(&mut self) {
			let slot = &mut self.frame[self.ab];
			drop(slot.take());
			let frame = self.tm.frame();
			assert_eq!(frame.frame_ab, self.ab);
			*slot = Some(frame);
			self.ab = !self.ab;
		}
	}

	#[test]
	fn test_gc() -> anyhow::Result<()> {
		let tm = TableSync::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let mut switch = FrameSwitch::new(tm.clone());
		let ti = &table.interface;
		ti.take();

		let slot1 = table.alloc_slot(Arc::new(42))?;
		let slot2 = table.alloc_slot(Arc::new(69))?;
		tm.flush();

		drop(slot1);
		assert_eq!(ti.take(), simple_empty());

		switch.switch();
		assert_eq!(ti.take(), simple_empty());

		drop(slot2);
		switch.switch();
		assert_eq!(ti.take(), &[&[0]]);

		switch.switch();
		assert_eq!(ti.take(), &[&[1]]);

		switch.switch();
		assert_eq!(ti.take(), simple_empty());
		switch.switch();
		assert_eq!(ti.take(), simple_empty());

		Ok(())
	}

	#[test]
	fn test_gc_long() -> anyhow::Result<()> {
		let tm = TableSync::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let ti = &table.interface;

		let a1 = tm.frame();
		assert_eq!(a1.frame_ab, A);
		let long_frame_b = tm.frame();
		assert_eq!(long_frame_b.frame_ab, B);
		drop(a1);

		drop(table.alloc_slot(Arc::new(42))?);
		tm.flush();
		assert_eq!(ti.take(), simple_empty());

		// doesn't matter how many frames, it never gets dropped until long_frame_b is done
		for _ in 0..5 {
			let a = tm.frame();
			assert_eq!(a.frame_ab, A);
			drop(a);
			// no cleanup happened
			assert_eq!(ti.take(), &[&[]; 0]);
		}

		// gc of nothing
		drop(long_frame_b);
		assert_eq!(ti.take(), simple_empty());

		// 2nd gc should drop 0
		drop(tm.frame());
		assert_eq!(ti.take(), &[&[0][..]]);

		Ok(())
	}

	#[test]
	fn test_gc_dry_out() -> anyhow::Result<()> {
		let tm = TableSync::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let ti = &table.interface;

		let a1 = tm.frame();
		drop(table.alloc_slot(Arc::new(42))?);
		tm.flush();
		drop(a1);
		assert_eq!(ti.take(), simple_empty());

		drop(tm.frame());
		assert_eq!(ti.take(), &[&[0][..]]);

		Ok(())
	}
}
