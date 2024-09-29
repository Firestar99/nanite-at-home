use crate::backend::ab::{ABArray, AB};
use crate::backend::range_set::DescriptorIndexRangeSet;
use crate::backend::slot_array::SlotArray;
use crate::backend::table_id::TABLE_COUNT;
use crate::sync::cell::UnsafeCell;
use crossbeam_queue::SegQueue;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, MutexGuard, RwLock};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{fence, AtomicU32};
use std::sync::Arc;
use vulkano_bindless_shaders::descriptor::{DescriptorId, DescriptorIndex, DescriptorType, DescriptorVersion};

pub trait TableInterface: 'static {
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet);
	fn flush(&self);
}

impl<T: TableInterface> TableInterface for Arc<T> {
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
		self.deref().drop_slots(indices);
	}

	fn flush(&self) {
		self.deref().flush();
	}
}

impl<T: TableInterface> TableInterface for Box<T> {
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
		self.deref().drop_slots(indices);
	}

	fn flush(&self) {
		self.deref().flush();
	}
}

pub struct TableManager {
	// TODO I hate this RwLock
	tables: [RwLock<Option<Table>>; TABLE_COUNT as usize],
	frame_mutex: Mutex<ABArray<u32>>,
	write_queue_ab: CachePadded<AtomicU32>,
}

struct Table {
	slots: SlotArray<TableSlot>,
	interface: Box<dyn TableInterface>,
	reaper_queue: ABArray<SegQueue<DescriptorIndex>>,
	dead_queue: SegQueue<DescriptorIndex>,
	next_free: CachePadded<AtomicU32>,
}

impl TableManager {
	pub fn new() -> Arc<Self> {
		Arc::new(TableManager {
			tables: core::array::from_fn(|_| RwLock::new(None)),
			frame_mutex: Mutex::new(ABArray::new(|| 0)),
			write_queue_ab: CachePadded::new(AtomicU32::new(AB::B.to_u32())),
		})
	}

	// FIXME replace TableId with DescriptorType?
	pub fn register<T: TableInterface>(
		&self,
		table_id: DescriptorType,
		slots_capacity: u32,
		interface: T,
	) -> Result<(), TableRegisterError> {
		let mut guard = self.tables[table_id.to_usize()].write();
		if let Some(_) = *guard {
			Err(TableRegisterError::TableAlreadyRegistered(table_id))
		} else {
			*guard = Some(Table {
				slots: SlotArray::new(slots_capacity),
				interface: Box::new(interface),
				reaper_queue: ABArray::new(|| SegQueue::new()),
				dead_queue: SegQueue::new(),
				next_free: CachePadded::new(AtomicU32::new(0)),
			});
			Ok(())
		}
	}

	#[inline]
	fn with_table<R>(&self, table_id: DescriptorType, f: impl FnOnce(&Table) -> R) -> R {
		let table = self.tables[table_id.to_usize()].read();
		if let Some(table) = table.as_ref() {
			f(table)
		} else {
			panic!("Invalid DescriptorType: table {:?} not registered", table_id)
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

	fn alloc_slot(self: &Arc<Self>, table: DescriptorType) -> Result<RcTableSlot, SlotAllocationError> {
		self.with_table(table, |t| unsafe {
			let index = if let Some(index) = t.dead_queue.pop() {
				Ok(index)
			} else {
				let index = t.next_free.fetch_add(1, Relaxed);
				if index < t.slots_capacity() {
					Ok(DescriptorIndex::new(index).unwrap())
				} else {
					Err(SlotAllocationError::NoMoreCapacity(t.slots_capacity()))
				}
			}?;
			let slot = &t.slots[index];
			slot.ref_count.store(1, Relaxed);
			let version = slot.read_version();
			let id = DescriptorId::new(table, index, version);
			Ok(RcTableSlot::new(Arc::into_raw(self.clone()), id))
		})
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
				.map(|table_lock| {
					let table = table_lock.read();
					table.as_ref().map(|table| {
						let reaper_queue = &table.reaper_queue[gc_queue];
						let mut set = DescriptorIndexRangeSet::new();
						while let Some(index) = reaper_queue.pop() {
							set.insert(index);
						}
						set
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

		for (table, gc_indices) in self.tables.iter().zip(table_gc_indices) {
			if let Some(gc_indices) = gc_indices {
				let table = table.read();
				if let Some(table) = table.as_ref() {
					table.interface.drop_slots(&gc_indices);

					for index in gc_indices.iter() {
						unsafe {
							let valid_version = table.slots[index].version.with_mut(|version| {
								*version += 1;
								DescriptorVersion::new(*version).is_some()
							});

							if valid_version {
								table.dead_queue.push(index);
							}
						}
					}
				} else {
					unreachable!();
				}
			}
		}
	}

	#[inline]
	fn ref_inc(&self, id: DescriptorId) {
		self.with_table(id.desc_type(), |t| {
			t.slots[id.index()].ref_count.fetch_add(1, Relaxed);
		})
	}

	#[inline]
	fn ref_dec(&self, id: DescriptorId) -> bool {
		self.with_table(id.desc_type(), |t| {
			match t.slots[id.index()].ref_count.fetch_sub(1, Relaxed) {
				0 => panic!("TableSlot ref_count underflow!"),
				1 => {
					fence(Acquire);
					t.reaper_queue[self.write_queue_ab()].push(id.index());
					true
				}
				_ => false,
			}
		})
	}
}

impl Table {
	#[inline]
	fn slots_capacity(&self) -> u32 {
		self.slots.len() as u32
	}
}

struct TableSlot {
	ref_count: AtomicU32,
	version: UnsafeCell<u32>,
}

impl TableSlot {
	/// # Safety
	/// creates a reference to `self.version`
	unsafe fn read_version(&self) -> DescriptorVersion {
		unsafe { DescriptorVersion::new(self.version.with(|v| *v)).unwrap() }
	}
}

impl Default for TableSlot {
	fn default() -> Self {
		Self {
			ref_count: AtomicU32::new(0),
			version: UnsafeCell::new(0),
		}
	}
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RcTableSlot {
	table_manager: *const TableManager,
	id: DescriptorId,
}

impl RcTableSlot {
	/// Creates a mew RcTableSlot
	///
	/// # Safety
	/// This function will take ownership of one `ref_count` increment of the slot.
	unsafe fn new(table_manager: *const TableManager, id: DescriptorId) -> Self {
		Self { table_manager, id }
	}

	fn table_manager(&self) -> &TableManager {
		unsafe { &*self.table_manager }
	}
}

impl Clone for RcTableSlot {
	fn clone(&self) -> Self {
		self.table_manager().ref_inc(self.id);
		unsafe { Self::new(self.table_manager, self.id) }
	}
}

impl Drop for RcTableSlot {
	fn drop(&mut self) {
		if self.table_manager().ref_dec(self.id) {
			// Safety: slot ref count hit 0, so decrement ref count of `TableManager` which was incremented in
			// `alloc_slot()` when this slot was created
			unsafe { drop(Arc::from_raw(self.table_manager)) };
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
	TableAlreadyRegistered(DescriptorType),
}

impl Error for TableRegisterError {}

impl Display for TableRegisterError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			TableRegisterError::TableAlreadyRegistered(table) => write!(f, "Table {:?} already registered", table),
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

	const TEST_TABLE: DescriptorType = unsafe { DescriptorType::new_unchecked(0) };

	struct DummyInterface;

	impl TableInterface for DummyInterface {
		fn drop_slots(&self, _indices: &DescriptorIndexRangeSet) {}

		fn flush(&self) {}
	}

	struct SimpleInterface {
		drops: Mutex<Vec<DescriptorIndexRangeSet>>,
	}

	impl SimpleInterface {
		pub fn new() -> Arc<Self> {
			Arc::new(Self {
				drops: Mutex::new(Vec::new()),
			})
		}

		pub fn take(&self) -> Vec<Vec<u32>> {
			take(&mut *self.drops.lock())
				.into_iter()
				.map(|set| set.iter().map(|i| i.to_u32()).collect())
				.collect()
		}
	}

	impl TableInterface for SimpleInterface {
		fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
			self.drops.lock().push(indices.clone());
		}

		fn flush(&self) {}
	}

	#[test]
	fn test_table_register() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, DummyInterface)?;
		Ok(())
	}

	#[test]
	fn test_table_double_register() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, DummyInterface)?;
		match tm.register(TEST_TABLE, 256, DummyInterface) {
			Ok(_) => panic!("expected Err from double registering the same table interface"),
			Err(_) => Ok(()),
		}
	}

	#[test]
	fn test_alloc_slot() -> anyhow::Result<()> {
		const N: u32 = 128;

		let tm = TableManager::new();
		tm.register(TEST_TABLE, N, DummyInterface)?;

		for i in 0..N {
			let slot = tm.alloc_slot(TEST_TABLE)?;
			assert_eq!(slot.id.index().to_u32(), i);
			assert_eq!(slot.id.desc_type(), TEST_TABLE);
			assert_eq!(slot.id.version().to_u32(), 0);
		}

		tm.alloc_slot(TEST_TABLE).expect_err("we should be out of slots");
		tm.alloc_slot(TEST_TABLE)
			.expect_err("asking again but still out of slots");

		Ok(())
	}

	#[test]
	fn test_slot_reuse() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, DummyInterface)?;

		let alloc = |cnt: u32, exp_offset: u32, exp_version: u32| {
			(0..cnt)
				.map(|i| {
					let slot = tm.alloc_slot(TEST_TABLE).unwrap();
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
		tm.register(TEST_TABLE, 128, DummyInterface)?;

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
		tm.register(TEST_TABLE, 128, DummyInterface)?;

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
		tm.register(TEST_TABLE, 128, DummyInterface)?;

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
		let mut ti = SimpleInterface::new();
		tm.register(TEST_TABLE, 128, ti.clone())?;
		let mut switch = FrameSwitch::new(tm.clone());
		ti.take();

		let slot1 = tm.alloc_slot(TEST_TABLE)?;
		let slot2 = tm.alloc_slot(TEST_TABLE)?;
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
		let mut ti = SimpleInterface::new();
		tm.register(TEST_TABLE, 128, ti.clone())?;

		let a1 = tm.frame();
		assert_eq!(a1.frame_ab, A);
		let long_frame_b = tm.frame();
		assert_eq!(long_frame_b.frame_ab, B);
		drop(a1);

		drop(tm.alloc_slot(TEST_TABLE)?);
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
		let mut ti = SimpleInterface::new();
		tm.register(TEST_TABLE, 128, ti.clone())?;

		let a1 = tm.frame();
		drop(tm.alloc_slot(TEST_TABLE)?);
		drop(a1);
		assert_eq!(ti.take(), &[&[], &[]]);

		drop(tm.frame());
		assert_eq!(ti.take(), &[&[0][..], &[]]);

		Ok(())
	}
}
