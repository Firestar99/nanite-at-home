use crate::backend::ab::{ABArray, AB};
use crate::backend::range_set::DescriptorIndexRangeSet;
use crate::backend::slot_array::SlotArray;
use crate::sync::cell::UnsafeCell;
use crossbeam_queue::SegQueue;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, MutexGuard, RwLock};
use static_assertions::const_assert_eq;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::mem::MaybeUninit;
use std::ops::{Deref, Index};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{fence, AtomicU32};
use std::sync::{Arc, Weak};
use vulkano_bindless_shaders::descriptor::{
	DescriptorId, DescriptorIndex, DescriptorType, DescriptorVersion, ID_TYPE_BITS,
};

pub trait TableInterface: 'static {
	type Slot;
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet);
	fn flush(&self);
}

pub const TABLE_COUNT: u32 = 1 << ID_TYPE_BITS;

pub struct TableManager {
	// TODO I hate this RwLock
	tables: [RwLock<Option<Weak<dyn AbstractTable>>>; TABLE_COUNT as usize],
	table_next_free: CachePadded<AtomicU32>,
	frame_mutex: Mutex<ABArray<u32>>,
	write_queue_ab: CachePadded<AtomicU32>,
}

unsafe impl Send for TableManager {}
unsafe impl Sync for TableManager {}

pub struct Table<I: TableInterface> {
	table_manager: Arc<TableManager>,
	table_id: DescriptorType,
	interface: I,
	slot_counters: SlotArray<SlotCounter>,
	slots: SlotArray<UnsafeCell<MaybeUninit<I::Slot>>>,
	reaper_queue: ABArray<SegQueue<DescriptorIndex>>,
	dead_queue: SegQueue<DescriptorIndex>,
	next_free: CachePadded<AtomicU32>,
}

unsafe impl<I: TableInterface> Send for Table<I> {}
unsafe impl<I: TableInterface> Sync for Table<I> {}

impl TableManager {
	pub fn new() -> Arc<Self> {
		Arc::new(TableManager {
			tables: core::array::from_fn(|_| RwLock::new(None)),
			table_next_free: CachePadded::new(AtomicU32::new(0)),
			frame_mutex: Mutex::new(ABArray::new(|| 0)),
			write_queue_ab: CachePadded::new(AtomicU32::new(AB::B.to_u32())),
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
				table_manager: self.clone(),
				table_id: unsafe { DescriptorType::new(table_id).unwrap() },
				interface,
				slot_counters: SlotArray::new(slots_capacity),
				slots: SlotArray::new_generator(slots_capacity, |_| UnsafeCell::new(MaybeUninit::uninit())),
				reaper_queue: ABArray::new(|| SegQueue::new()),
				dead_queue: SegQueue::new(),
				next_free: CachePadded::new(AtomicU32::new(0)),
			});
			let old_table = guard.replace(Arc::downgrade(&(table.clone() as Arc<dyn AbstractTable>)));
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
		let table_gc_indices;
		{
			let gc_queue = !dropped_frame_ab;
			table_gc_indices = self
				.tables
				.iter()
				.filter_map(|table_lock| {
					let table = table_lock.read();
					table.as_ref().and_then(|table| {
						if let Some(table) = table.upgrade() {
							Some((table.gc_collect(gc_queue), table))
						} else {
							None
						}
					})
				})
				.collect::<Vec<_>>();

			// Release may seem a bit defensive here, as we don't actually need to flush any memory.
			// But it ensures that when creating a new FrameGuard afterward and sending it to another thread via
			// Rel/Acq, this write is visible. Which is important as it could otherwise write to be gc'ed objects to the
			// wrong queue.
			self.write_queue_ab.store(gc_queue.to_u32(), Release);
			drop(guard);
		}

		for (gc_indices, table) in table_gc_indices {
			table.gc_drop(gc_indices)
		}
	}
}

impl<I: TableInterface> Table<I> {
	#[inline]
	pub fn slots_capacity(&self) -> u32 {
		self.slot_counters.len() as u32
	}

	pub fn alloc_slot(self: &Arc<Self>, slot: I::Slot) -> Result<RcTableSlot<I>, SlotAllocationError> {
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
		unsafe {
			self.slots[index].with_mut(|s| {
				s.write(slot);
			})
		}
		let slot = &self.slot_counters[index];
		slot.ref_count.store(1, Release);

		// Safety: this is a valid id, we transfer the ref_count inc above to the RcTableSlot
		unsafe {
			let id = DescriptorId::new(self.table_id, index, slot.read_version());
			Ok(RcTableSlot::new(Arc::into_raw(self.clone()), id))
		}
	}

	#[inline]
	fn ref_inc(&self, id: DescriptorId) {
		self.slot_counters[id.index()].ref_count.fetch_add(1, Relaxed);
	}

	#[inline]
	fn ref_dec(&self, id: DescriptorId) -> bool {
		match self.slot_counters[id.index()].ref_count.fetch_sub(1, Relaxed) {
			0 => panic!("TableSlot ref_count underflow!"),
			1 => {
				fence(Acquire);
				self.reaper_queue[self.table_manager.write_queue_ab()].push(id.index());
				true
			}
			_ => false,
		}
	}
}

impl<I: TableInterface> Deref for Table<I> {
	type Target = I;

	fn deref(&self) -> &Self::Target {
		&self.interface
	}
}

trait AbstractTable {
	fn gc_collect(&self, gc_queue: AB) -> DescriptorIndexRangeSet;
	fn gc_drop(&self, gc_indices: DescriptorIndexRangeSet);
}

impl<I: TableInterface> AbstractTable for Table<I> {
	fn gc_collect(&self, gc_queue: AB) -> DescriptorIndexRangeSet {
		let reaper_queue = &self.reaper_queue[gc_queue];
		let mut set = DescriptorIndexRangeSet::new();
		while let Some(index) = reaper_queue.pop() {
			set.insert(index);
		}
		set
	}

	fn gc_drop(&self, gc_indices: DescriptorIndexRangeSet) {
		self.interface.drop_slots(&gc_indices);

		for i in gc_indices.iter() {
			// Safety: we have exclusive access to the previously initialized slot
			let valid_version = unsafe {
				self.slots.index(i).with_mut(|s| s.assume_init_drop());
				self.slot_counters[i].version.with_mut(|version| {
					*version += 1;
					DescriptorVersion::new(*version).is_some()
				})
			};

			// we send / share the slot to the dead_queue
			if valid_version {
				self.dead_queue.push(i);
			}
		}
	}
}

impl<I: TableInterface> Drop for Table<I> {
	fn drop(&mut self) {
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
		unsafe { DescriptorVersion::new(self.version.with(|v| *v)).unwrap() }
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
pub struct RcTableSlot<I: TableInterface> {
	table: *const Table<I>,
	id: DescriptorId,
}

unsafe impl<I: TableInterface> Send for RcTableSlot<I> {}
unsafe impl<I: TableInterface> Sync for RcTableSlot<I> {}

impl<I: TableInterface> RcTableSlot<I> {
	/// Creates a mew RcTableSlot
	///
	/// # Safety
	/// This function will take ownership of one `ref_count` increment of the slot.
	#[inline]
	unsafe fn new(table: *const Table<I>, id: DescriptorId) -> Self {
		Self { table, id }
	}

	#[inline]
	pub fn table(&self) -> &Table<I> {
		unsafe { &*self.table }
	}

	#[inline]
	pub fn id(&self) -> DescriptorId {
		self.id
	}
}

impl<I: TableInterface> Deref for RcTableSlot<I> {
	type Target = I::Slot;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { (&*self.table().slots.index(self.id.index()).get()).assume_init_ref() }
	}
}

impl<I: TableInterface> Debug for RcTableSlot<I> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RcTableSlot")
			.field("table_id", &self.table().table_id.to_u32())
			.field("id", &self.id)
			.finish()
	}
}

impl<I: TableInterface> Clone for RcTableSlot<I> {
	#[inline]
	fn clone(&self) -> Self {
		self.table().ref_inc(self.id);
		unsafe { Self::new(self.table, self.id) }
	}
}

impl<I: TableInterface> Drop for RcTableSlot<I> {
	#[inline]
	fn drop(&mut self) {
		if self.table().ref_dec(self.id) {
			// Safety: slot ref count hit 0, so decrement ref count of `TableManager` which was incremented in
			// `alloc_slot()` when this slot was created
			unsafe { drop(Arc::from_raw(self.table)) };
		}
	}
}

pub struct FrameGuard {
	table_manager: Arc<TableManager>,
	frame_ab: AB,
}

impl FrameGuard {
	pub fn table_manager(&self) -> &Arc<TableManager> {
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
	use crate::backend::ab::AB::*;
	use std::mem::take;

	struct DummyInterface;

	impl TableInterface for DummyInterface {
		type Slot = Arc<u32>;

		fn drop_slots(&self, _indices: &DescriptorIndexRangeSet) {}

		fn flush(&self) {}
	}

	struct SimpleInterface {
		drops: Mutex<Vec<DescriptorIndexRangeSet>>,
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
				.map(|set| set.iter().map(|i| i.to_u32()).collect())
				.collect()
		}
	}

	impl TableInterface for SimpleInterface {
		type Slot = Arc<u32>;

		fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
			self.drops.lock().push(indices.clone());
		}

		fn flush(&self) {}
	}

	#[test]
	fn test_table_register() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(128, DummyInterface)?;
		Ok(())
	}

	#[test]
	fn test_alloc_slot() -> anyhow::Result<()> {
		const N: u32 = 128;

		let tm = TableManager::new();
		let table = tm.register(N, DummyInterface)?;

		let _slots = (0..N)
			.map(|i| {
				let slot = table.alloc_slot(Arc::new(42 + i)).unwrap();
				assert_eq!(slot.id.index().to_u32(), i);
				assert_eq!(slot.id.desc_type().to_u32(), 0);
				assert_eq!(slot.id.version().to_u32(), 0);
				assert_eq!(**slot, 42 + i);
				slot
			})
			.collect::<Vec<_>>();

		table.alloc_slot(Arc::new(69)).expect_err("we should be out of slots");
		table
			.alloc_slot(Arc::new(70))
			.expect_err("asking again but still out of slots");

		Ok(())
	}

	#[test]
	fn test_slot_reuse() -> anyhow::Result<()> {
		let tm = TableManager::new();
		let table = tm.register(128, DummyInterface)?;

		let alloc = |cnt: u32, exp_offset: u32, exp_version: u32| {
			(0..cnt)
				.map(|i| {
					let slot = table.alloc_slot(Arc::new(42 + i)).unwrap();
					assert_eq!(**slot, 42 + i);
					assert_eq!(slot.id.index().to_u32(), i + exp_offset);
					assert_eq!(slot.id.version().to_u32(), exp_version);
					slot
				})
				.collect::<Vec<_>>()
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
		let tm = TableManager::new();
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
		let tm = TableManager::new();
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
		let tm = TableManager::new();
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
		tm: Arc<TableManager>,
		frame: ABArray<Option<FrameGuard>>,
		ab: AB,
	}

	impl FrameSwitch {
		pub fn new(tm: Arc<TableManager>) -> Self {
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
		let tm = TableManager::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let mut switch = FrameSwitch::new(tm.clone());
		let ti = &table.interface;
		ti.take();

		let slot1 = table.alloc_slot(Arc::new(42))?;
		let slot2 = table.alloc_slot(Arc::new(69))?;
		drop(slot1);
		assert_eq!(ti.take(), Vec::<Vec<u32>>::new());

		switch.switch();
		assert_eq!(ti.take(), &[&[]]);

		drop(slot2);
		switch.switch();
		assert_eq!(ti.take(), &[&[0]]);

		switch.switch();
		assert_eq!(ti.take(), &[&[1]]);

		switch.switch();
		assert_eq!(ti.take(), &[&[]]);
		switch.switch();
		assert_eq!(ti.take(), &[&[]]);

		Ok(())
	}

	#[test]
	fn test_gc_long() -> anyhow::Result<()> {
		let tm = TableManager::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let ti = &table.interface;

		let a1 = tm.frame();
		assert_eq!(a1.frame_ab, A);
		let long_frame_b = tm.frame();
		assert_eq!(long_frame_b.frame_ab, B);
		drop(a1);

		drop(table.alloc_slot(Arc::new(42))?);
		assert_eq!(ti.take(), &[&[], &[]]);

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
		assert_eq!(ti.take(), &[&[]]);

		// 2nd gc should drop 0
		drop(tm.frame());
		assert_eq!(ti.take(), &[&[0][..], &[]]);

		Ok(())
	}

	#[test]
	fn test_gc_dry_out() -> anyhow::Result<()> {
		let tm = TableManager::new();
		let table = tm.register(128, SimpleInterface::new())?;
		let ti = &table.interface;

		let a1 = tm.frame();
		drop(table.alloc_slot(Arc::new(42))?);
		drop(a1);
		assert_eq!(ti.take(), &[&[], &[]]);

		drop(tm.frame());
		assert_eq!(ti.take(), &[&[0][..], &[]]);

		Ok(())
	}
}
