use crate::backend::ab::{ABArray, AB};
use crate::backend::range_set::DescriptorIndexRangeSet;
use crate::backend::table_id::{TableId, TABLE_COUNT};
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

pub trait TableInterface {
	fn drop_slots(&self, indices: &DescriptorIndexRangeSet);
	fn flush(&self);
}

pub struct TableManager {
	// TODO I hate this RwLock
	tables: [RwLock<Option<Table>>; TABLE_COUNT as usize],
	frame_mutex: Mutex<ABArray<u32>>,
	write_queue_ab: CachePadded<AtomicU32>,
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
	pub fn register<T: TableInterface>(&self, id: TableId, slots_capacity: u32, interface: T) -> Result<(), ()> {
		let guard = self.tables[id.to_usize()].write();
		if let Some(_) = *guard {
			Err(())
		} else {
			*guard = Some(Table::new(slots_capacity, Box::new(interface)));
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
			let index = t.alloc_slot()?;
			let id = DescriptorId::new(table, index, t.version(index));
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
					table.as_ref().map(|table| table.gc_queue_collect(gc_queue))
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
					table.gc_queue_drop(gc_indices);
				} else {
					unreachable!();
				}
			}
		}
	}
}

struct Table {
	slots: Box<[TableSlot]>,
	interface: Box<dyn TableInterface>,
	reaper_queue: ABArray<SegQueue<DescriptorIndex>>,
	dead_queue: SegQueue<DescriptorIndex>,
	next_free: CachePadded<AtomicU32>,
}

impl Table {
	fn new(slots_capacity: u32, interface: Box<dyn TableInterface>) -> Self {
		Self {
			slots: (0..slots_capacity)
				.map(|_| TableSlot::default())
				.collect::<Vec<_>>()
				.into_boxed_slice(),
			interface,
			reaper_queue: ABArray::new(|| SegQueue::new()),
			dead_queue: SegQueue::new(),
			next_free: CachePadded::new(AtomicU32::new(0)),
		}
	}

	#[inline]
	fn slots_capacity(&self) -> u32 {
		self.slots.len() as u32
	}

	#[inline]
	fn alloc_slot(&self) -> Result<DescriptorIndex, SlotAllocationError> {
		if let Some(index) = self.dead_queue.pop() {
			Ok(index)
		} else {
			let index = self.next_free.fetch_add(1, Relaxed);
			if index < self.slots_capacity() {
				Ok(unsafe { DescriptorIndex::new(index).unwrap() })
			} else {
				Err(SlotAllocationError::NoMoreCapacity(self.slots_capacity()))
			}
		}
	}

	fn version(&self, index: DescriptorIndex) -> DescriptorVersion {
		unsafe { DescriptorVersion::new(self.slot(index).version.with(|v| *v)).unwrap() }
	}

	fn gc_queue_collect(&self, ab: AB) -> DescriptorIndexRangeSet {
		let reaper_queue = &self.reaper_queue[ab];
		let mut set = DescriptorIndexRangeSet::new();
		while let Some(index) = reaper_queue.pop() {
			set.insert(index..index);
		}
		set
	}

	fn gc_queue_drop(&self, indices: DescriptorIndexRangeSet) {
		self.interface.drop_slots(&indices);

		for index in indices.iter() {
			let slot = self.slot(index);
			unsafe {
				let valid_version = slot.version.with_mut(|version| {
					*version += 1;
					DescriptorVersion::new(*version).is_some()
				});

				if valid_version {
					self.dead_queue.push(index);
				}
			}
		}
	}

	#[inline]
	fn slot(&self, index: DescriptorIndex) -> &TableSlot {
		&self.slots[index.to_usize()]
	}

	#[inline]
	fn ref_inc(&self, index: DescriptorIndex) {
		self.slot(index).ref_count.fetch_add(1, Relaxed);
	}

	#[inline]
	fn ref_dec(&self, index: DescriptorIndex) {
		match self.slot(index).ref_count.fetch_sub(1, Relaxed) {
			0 => panic!("TableSlot ref_count underflow!"),
			1 => {
				fence(Acquire);
				self.slot_starts_dying(index);
			}
			_ => (),
		}
	}

	#[cold]
	#[inline(never)]
	fn slot_starts_dying(&self, index: DescriptorIndex) {
		let slot = self.slot(index);
		// TODO insert to reaper, version inc here or later?
	}
}

struct TableSlot {
	ref_count: AtomicU32,
	version: UnsafeCell<u32>,
}

impl Default for TableSlot {
	fn default() -> Self {
		Self {
			ref_count: AtomicU32::new(0),
			version: UnsafeCell::new(0),
		}
	}
}

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
		self.table_manager()
			.with_table(self.id.desc_type(), |t| t.ref_inc(self.id.index()));
		unsafe { Self::new(self.table_manager, self.id) }
	}
}

impl Drop for RcTableSlot {
	fn drop(&mut self) {
		self.table_manager()
			.with_table(self.id.desc_type(), |t| t.ref_dec(self.id.index()));
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
