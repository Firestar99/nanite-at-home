use crate::backend::ab::{ABArray, AB};
use crate::backend::range_set::DescriptorIndexRangeSet;
use crate::backend::table_id::TABLE_COUNT;
use crate::sync::cell::UnsafeCell;
use crossbeam_queue::SegQueue;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, MutexGuard, RwLock};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{fence, AtomicU32};
use std::sync::Arc;
use vulkano_bindless_shaders::descriptor::{DescriptorId, DescriptorIndex, DescriptorType, DescriptorVersion};

pub trait TableInterface: 'static {
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet);
	fn flush(&self);
}

pub struct TableManager {
	// TODO I hate this RwLock
	tables: [RwLock<Option<Table>>; TABLE_COUNT as usize],
	frame_mutex: Mutex<ABArray<u32>>,
	write_queue_ab: CachePadded<AtomicU32>,
}

struct Table {
	slots: Box<[TableSlot]>,
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
			write_queue_ab: CachePadded::new(AtomicU32::new(AB::A.to_u32())),
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
				slots: (0..slots_capacity)
					.map(|_| TableSlot::default())
					.collect::<Vec<_>>()
					.into_boxed_slice(),
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
			let slot = t.slot(index);
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
			// note the negation!
			frame_ab = !self.write_queue_ab();
			guard[frame_ab] += 1;
		}

		FrameGuard {
			table_manager: self.clone(),
			frame_ab,
		}
	}

	fn frame_drop(self: &Arc<Self>, frame_ab: AB) {
		let mut guard = self.frame_mutex.lock();
		let frame_cnt = &mut guard[frame_ab];
		match *frame_cnt {
			0 => panic!("frame ref counting underflow"),
			1 => {
				*frame_cnt = 0;
				self.last_frame_finished(guard, frame_ab);
			}
			_ => *frame_cnt -= 1,
		}
	}

	#[cold]
	#[inline(never)]
	fn last_frame_finished(&self, guard: MutexGuard<ABArray<u32>>, frame_ab: AB) {
		let table_gc_indices;
		{
			let write_queue_ab = self.write_queue_ab();
			// note the double inversion
			if !write_queue_ab != frame_ab {
				return;
			}

			let gc_queue = !frame_ab;
			table_gc_indices = self
				.tables
				.iter()
				.map(|table_lock| {
					let table = table_lock.read();
					table.as_ref().map(|table| self.gc_queue_collect(table, gc_queue))
				})
				.collect::<Vec<_>>();

			// TODO Release is a bit defensive here.
			self.write_queue_ab.store(gc_queue.to_u32(), Release);
			drop(guard);
		}

		for (table, gc_indices) in self.tables.iter().zip(table_gc_indices) {
			if let Some(gc_indices) = gc_indices {
				let table = table.read();
				if let Some(table) = table.as_ref() {
					self.gc_queue_drop(table, gc_indices);
				} else {
					unreachable!();
				}
			}
		}
	}

	fn gc_queue_collect(&self, table: &Table, ab: AB) -> DescriptorIndexRangeSet {
		let reaper_queue = &table.reaper_queue[ab];
		let mut set = DescriptorIndexRangeSet::new();
		while let Some(index) = reaper_queue.pop() {
			set.insert(index);
		}
		set
	}

	fn gc_queue_drop(&self, table: &Table, indices: DescriptorIndexRangeSet) {
		table.interface.drop_slots(&indices);

		for index in indices.iter() {
			let slot = table.slot(index);
			unsafe {
				let valid_version = slot.version.with_mut(|version| {
					*version += 1;
					DescriptorVersion::new(*version).is_some()
				});

				if valid_version {
					table.dead_queue.push(index);
				}
			}
		}
	}

	#[inline]
	fn ref_inc(&self, id: DescriptorId) {
		self.with_table(id.desc_type(), |t| {
			let slot = t.slot(id.index());
			slot.ref_count.fetch_add(1, Relaxed);
		})
	}

	#[inline]
	fn ref_dec(&self, id: DescriptorId) {
		self.with_table(id.desc_type(), |t| {
			let slot = t.slot(id.index());
			match slot.ref_count.fetch_sub(1, Relaxed) {
				0 => panic!("TableSlot ref_count underflow!"),
				1 => {
					fence(Acquire);
					t.reaper_queue[self.write_queue_ab()].push(id.index());
				}
				_ => (),
			}
		})
	}
}

impl Table {
	#[inline]
	fn slot(&self, index: DescriptorIndex) -> &TableSlot {
		&self.slots[index.to_usize()]
	}

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

#[derive(Debug)]
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
		self.table_manager().ref_dec(self.id);
	}
}

pub struct FrameGuard {
	table_manager: Arc<TableManager>,
	frame_ab: AB,
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

	const TEST_TABLE: DescriptorType = unsafe { DescriptorType::new_unchecked(0) };

	struct TestInterface {
		drops: Mutex<Vec<DescriptorIndexRangeSet>>,
	}

	impl TestInterface {
		pub fn new() -> Self {
			Self {
				drops: Mutex::new(Vec::new()),
			}
		}
	}

	impl TableInterface for TestInterface {
		fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
			self.drops.lock().push(indices.clone());
		}

		fn flush(&self) {}
	}

	#[test]
	fn test_table_register() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, TestInterface::new())?;
		Ok(())
	}

	#[test]
	fn test_table_double_register() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, TestInterface::new())?;
		match tm.register(TEST_TABLE, 256, TestInterface::new()) {
			Ok(_) => panic!("expected Err from double registering the same table interface"),
			Err(_) => Ok(()),
		}
	}

	#[test]
	fn test_alloc_slot() -> anyhow::Result<()> {
		const N: u32 = 128;

		let tm = TableManager::new();
		tm.register(TEST_TABLE, N, TestInterface::new())?;

		for i in 0..N {
			let slot = tm.alloc_slot(TEST_TABLE)?;
			assert_eq!(slot.id.index().to_u32(), i);
			assert_eq!(slot.id.desc_type(), TEST_TABLE);
			assert_eq!(slot.id.version().to_u32(), 0);
		}

		tm.alloc_slot(TEST_TABLE).expect_err("we should be out of slots");

		Ok(())
	}

	#[test]
	fn test_slot_reuse() -> anyhow::Result<()> {
		let tm = TableManager::new();
		tm.register(TEST_TABLE, 128, TestInterface::new())?;

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
		let flush = || drop(tm.frame());

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
}
