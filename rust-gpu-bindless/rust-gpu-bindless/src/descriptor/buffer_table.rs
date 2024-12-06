use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::boxed::{BoxDesc, BoxDescExt};
use crate::descriptor::buffer_metadata_cpu::StrongMetadataCpu;
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::{AnyRCDesc, Bindless, BindlessAllocationScheme, DescContentMutCpu, DescriptorCounts};
use crate::platform::BindlessPlatform;
use parking_lot::Mutex;
use rust_gpu_bindless_shaders::buffer_content::Metadata;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, MutBuffer};
use smallvec::SmallVec;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Deref;
use std::sync::{Arc, Weak};

impl<T: BufferContent + ?Sized> DescContentCpu for Buffer<T> {
	type DescTable<P: BindlessPlatform> = BufferTable<P>;
	type VulkanType<P: BindlessPlatform> = P::TypedBuffer<T::Transfer>;
	type Slot<P: BindlessPlatform> = BufferSlot<P>;

	fn get_slot<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::Slot<P> {
		&slot.try_deref::<BufferInterface<P>>().unwrap()
	}

	fn deref_table<P: BindlessPlatform>(slot: &Self::Slot<P>) -> &Self::VulkanType<P> {
		unsafe { P::reinterpet_ref_buffer(&slot.buffer) }
	}
}

// TODO should this be deduplicated by moving some methods to BufferTable?
impl<T: BufferContent + ?Sized> DescContentCpu for MutBuffer<T> {
	type DescTable<P: BindlessPlatform> = BufferTable<P>;
	type VulkanType<P: BindlessPlatform> = P::TypedBuffer<T::Transfer>;
	type Slot<P: BindlessPlatform> = BufferSlot<P>;

	fn get_slot<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::Slot<P> {
		&slot.try_deref::<BufferInterface<P>>().unwrap()
	}

	fn deref_table<P: BindlessPlatform>(slot: &Self::Slot<P>) -> &Self::VulkanType<P> {
		unsafe { P::reinterpet_ref_buffer(&slot.buffer) }
	}
}

impl<T: BufferContent + ?Sized> DescContentMutCpu for MutBuffer<T> {
	type Shared = Buffer<T>;
}

impl<P: BindlessPlatform> DescTable for BufferTable<P> {}

pub struct BufferSlot<P: BindlessPlatform> {
	pub buffer: P::Buffer,
	/// len in T's if this is a slice, otherwise 1
	pub len: usize,
	/// the total size of this buffer in bytes
	pub size: u64,
	pub usage: BindlessBufferUsage,
	pub memory_allocation: P::MemoryAllocation,
	pub strong_refs: Mutex<StrongBackingRefs<P>>,
}

pub struct BufferTable<P: BindlessPlatform> {
	table: Arc<Table<BufferInterface<P>>>,
}

impl<P: BindlessPlatform> BufferTable<P> {
	pub fn new(table_sync: &Arc<TableSync>, counts: DescriptorCounts, bindless: Weak<Bindless<P>>) -> Self {
		Self {
			table: table_sync
				.register(counts.buffers, BufferInterface { bindless })
				.unwrap(),
		}
	}
}

pub struct BufferInterface<P: BindlessPlatform> {
	bindless: Weak<Bindless<P>>,
}

impl<P: BindlessPlatform> TableInterface for BufferInterface<P> {
	type Slot = BufferSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			if let Some(bindless) = self.bindless.upgrade() {
				bindless
					.platform
					.destroy_buffers(&bindless.global_descriptor_set(), indices);
			}
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}

pub struct BufferTableAccess<'a, P: BindlessPlatform>(pub &'a Arc<Bindless<P>>);

impl<'a, P: BindlessPlatform> Deref for BufferTableAccess<'a, P> {
	type Target = BufferTable<P>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.buffer
	}
}

bitflags::bitflags! {
	/// Buffer usage specify how you may use a buffer. Missing flags are only validated during runtime.
	#[repr(transparent)]
	#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
	pub struct BindlessBufferUsage: u64 {
		/// Can be used as a source of transfer operations
		const TRANSFER_SRC = 0b1;
		/// Can be used as a destination of transfer operations
		const TRANSFER_DST = 0b10;
		/// Allows a buffer to be mapped into host memory. The mapping will optimize for reading from the device.
		const MAP_READ = 0b100;
		/// Allows a buffer to be mapped into host memory. The mapping will optimize for writing to the device.
		const MAP_WRITE = 0b1000;
		/// Can be used as uniform buffer
		const UNIFORM_BUFFER = 0b1_0000;
		/// Can be used as storage buffer
		const STORAGE_BUFFER = 0b10_0000;
		/// Can be used as source of fixed-function index fetch (index buffer)
		const INDEX_BUFFER = 0b100_0000;
		/// Can be used as source of fixed-function vertex fetch (VBO)
		const VERTEX_BUFFER = 0b1000_0000;
		/// Can be the source of indirect parameters (e.g. indirect buffer, parameter buffer)
		const INDIRECT_BUFFER = 0b1_0000_0000;
	}
}

impl BindlessBufferUsage {
	pub fn is_mapped(&self) -> bool {
		self.contains(BindlessBufferUsage::MAP_READ) || self.contains(BindlessBufferUsage::MAP_WRITE)
	}
}

pub struct BindlessBufferCreateInfo<'a> {
	/// Buffer usage specify how you may use a buffer. Missing flags are only validated during runtime.
	pub usage: BindlessBufferUsage,
	/// Determines how this allocation should be managed.
	pub allocation_scheme: BindlessAllocationScheme,
	/// Name of the buffer, for tracking and debugging purposes
	pub name: &'a str,
}

impl<'a, P: BindlessPlatform> BufferTableAccess<'a, P> {
	/// Allocates a new slot for the supplied buffer
	///
	/// # Safety
	/// The Buffer's device must be the same as the bindless device. Ownership of the buffer is transferred to this
	/// table. You may not access or drop it afterward, except by going though the returned `RCDesc`.
	/// The generic T must match the contents of the Buffer and the size of the buffer must not be smaller than T.
	#[inline]
	pub unsafe fn alloc_slot<T: BufferContent + ?Sized>(&self, buffer: BufferSlot<P>) -> BoxDesc<P, MutBuffer<T>> {
		unsafe {
			BoxDesc::new(
				self.table
					.alloc_slot(buffer)
					.map_err(|a| format!("BufferTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, BufferInterface<P>> {
		self.table.drain_flush_queue()
	}

	pub fn alloc_sized<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
	) -> Result<BoxDesc<P, MutBuffer<T>>, P::AllocationError> {
		unsafe {
			let size = size_of::<T::Transfer>() as u64;
			let (buffer, memory_allocation) = self.0.platform.alloc_buffer(create_info, size)?;
			Ok(self.alloc_slot(BufferSlot {
				buffer,
				len: 1,
				size,
				usage: create_info.usage,
				memory_allocation,
				strong_refs: Default::default(),
			}))
		}
	}

	pub fn alloc_slice<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		len: usize,
	) -> Result<BoxDesc<P, MutBuffer<[T]>>, P::AllocationError> {
		unsafe {
			let size = size_of::<T::Transfer>() as u64 * len as u64;
			let (buffer, memory_allocation) = self.0.platform.alloc_buffer(create_info, size)?;
			Ok(self.alloc_slot(BufferSlot {
				buffer,
				len,
				size,
				usage: create_info.usage,
				memory_allocation,
				strong_refs: Default::default(),
			}))
		}
	}

	pub fn alloc_from_data<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		data: T,
	) -> Result<BoxDesc<P, MutBuffer<T>>, P::AllocationError> {
		let mut buffer = self.alloc_sized(create_info)?;
		unsafe { buffer.mapped().write_data(data) };
		Ok(buffer)
	}

	pub fn alloc_from_iter<T: BufferStruct, I>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		iter: I,
	) -> Result<BoxDesc<P, MutBuffer<[T]>>, P::AllocationError>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let iter = iter.into_iter();
		let mut buffer = self.alloc_slice(create_info, iter.len())?;
		unsafe { buffer.mapped().overwrite_from_iter_exact(iter) };
		Ok(buffer)
	}
}

pub trait MutDescBufferExt<P: BindlessPlatform, T: BufferContent + ?Sized> {
	/// Map and access the buffer's contents on the host
	///
	/// # Safety
	/// Buffer must not be in use simultaneously by the Device
	unsafe fn mapped(&mut self) -> MappedBuffer<P, T>;
}

impl<P: BindlessPlatform, T: BufferContent + ?Sized> MutDescBufferExt<P, T> for BoxDesc<P, MutBuffer<T>> {
	unsafe fn mapped(&mut self) -> MappedBuffer<P, T> {
		MappedBuffer {
			table_sync: self.rc_slot().table_sync_arc(),
			slot: self.inner_slot(),
			_phantom: PhantomData,
		}
	}
}

pub struct MappedBuffer<'a, P: BindlessPlatform, T: BufferContent + ?Sized> {
	table_sync: Arc<TableSync>,
	slot: &'a BufferSlot<P>,
	_phantom: PhantomData<T>,
}

impl<'a, P: BindlessPlatform, T: BufferContent> MappedBuffer<'a, P, T> {
	/// Assume that the **following** operations will completely overwrite the buffer.
	///
	/// # Safety
	/// Failing to completely overwrite the buffer may allow the shader to read dangling [`StrongDesc`], potentially
	/// causing memory errors.
	pub unsafe fn assume_will_overwrite_completely(&self) {
		*self.slot.strong_refs.lock() = StrongBackingRefs::default();
	}
}

impl<'a, P: BindlessPlatform, T: BufferStruct> MappedBuffer<'a, P, T> {
	/// Copy data `T` to buffer. Implicitly, it will fully overwrite the buffer.
	pub fn write_data(&mut self, t: T) {
		unsafe {
			let mut meta = StrongMetadataCpu::new(&self.table_sync, Metadata {});
			let slab = P::memory_allocation_to_slab(&self.slot.memory_allocation);
			let record = presser::copy_from_slice_to_offset(&[T::write_cpu(t, &mut meta)], slab, 0).unwrap();
			assert_eq!(record.copy_start_offset, 0, "presser must not add padding");
			*self.slot.strong_refs.lock() = meta.into_backing_refs();
		}
	}
}

impl<'a, P: BindlessPlatform, T: BufferStruct> MappedBuffer<'a, P, [T]> {
	pub fn len(&self) -> usize {
		self.slot.len
	}

	/// Copy from `iter` of `T` into the Buffer slice.
	/// The iterator must yield exactly [`Self::len`] amount of elements to fully overwrite the buffer, otherwise this
	/// function panics.
	pub fn overwrite_from_iter_exact(&mut self, iter: impl Iterator<Item = T>) {
		unsafe {
			let mut meta = StrongMetadataCpu::new(&self.table_sync, Metadata {});
			let slab = P::memory_allocation_to_slab(&self.slot.memory_allocation);
			let mut written = 0;
			let record = presser::copy_from_iter_to_offset_with_align_packed(
				iter.map(|i| {
					written += 1;
					T::write_cpu(i, &mut meta)
				}),
				slab,
				0,
				1,
			);
			assert_eq!(
				written, self.slot.len,
				"Iterator did not yield exactly {} elements!",
				self.slot.len
			);
			if let Some(record) = record.unwrap() {
				assert_eq!(record.copy_start_offset, 0, "presser must not add padding");
			}
			*self.slot.strong_refs.lock() = meta.into_backing_refs();
		}
	}

	/// Copy from `iter` of `T` into the Buffer slice.
	/// If the iterator yields too few elements to fill the buffer, the remaining elements are filled
	pub fn overwrite_from_iter_and_fill_with(&mut self, iter: impl Iterator<Item = T>, fill: impl FnMut() -> T) {
		self.overwrite_from_iter_exact(iter.chain(std::iter::repeat_with(fill)).take(self.slot.len))
	}

	/// Write `t` at a certain `index` in this slice.
	///
	/// # StrongRefs
	/// Note that any `StrongDesc` previously referenced from this `index`'s `T` will **NOT** be deallocated
	/// until you use any of the `overwrite*` functions to fully reinitialize this buffer. You can also use the unsafe
	/// [`Self::assume_will_overwrite_completely`] to invalidate all previous `StrongDesc` within the buffer.
	pub fn write_offset(&mut self, index: usize, t: T) {
		unsafe {
			let mut meta = StrongMetadataCpu::new(&self.table_sync, Metadata {});
			let slab = P::memory_allocation_to_slab(&self.slot.memory_allocation);
			let record = presser::copy_from_slice_to_offset(
				&[T::write_cpu(t, &mut meta)],
				slab,
				index * size_of::<T::Transfer>(),
			)
			.unwrap();
			assert_eq!(record.copy_start_offset, 0, "presser must not add padding");
			self.slot.strong_refs.lock().merge(meta.into_backing_refs());
		}
	}
}

impl<'a, P: BindlessPlatform, T: BufferStruct + Clone> MappedBuffer<'a, P, [T]> {
	/// Copy from `iter` of `T` into the Buffer slice.
	/// If the iterator yields too few elements to fill the buffer, the remaining elements are filled
	pub fn overwrite_from_iter_and_fill(&mut self, iter: impl Iterator<Item = T>, fill: T) {
		self.overwrite_from_iter_exact(iter.chain(std::iter::repeat(fill)).take(self.slot.len))
	}
}

impl<'a, P: BindlessPlatform, T: BufferStruct + Default> MappedBuffer<'a, P, [T]> {
	/// Copy from `iter` of `T` into the Buffer slice.
	/// If the iterator yields too few elements to fill the buffer, the remaining elements are filled with
	/// [`Default::default`]
	pub fn overwrite_from_iter_and_fill_default(&mut self, iter: impl Iterator<Item = T>) {
		self.overwrite_from_iter_and_fill_with(iter, Default::default)
	}
}

/// Stores [`RC`] to various resources, to which [`StrongDesc`] contained in some resource may refer to.
pub struct StrongBackingRefs<P: BindlessPlatform>(pub SmallVec<[AnyRCDesc<P>; 5]>);

impl<P: BindlessPlatform> Clone for StrongBackingRefs<P> {
	fn clone(&self) -> Self {
		Self { 0: self.0.clone() }
	}
}

impl<P: BindlessPlatform> Default for StrongBackingRefs<P> {
	fn default() -> Self {
		Self(SmallVec::default())
	}
}

impl<P: BindlessPlatform> StrongBackingRefs<P> {
	pub fn merge(&mut self, mut other: Self) {
		if self.0.is_empty() {
			self.0 = other.0;
		} else {
			self.0.append(&mut other.0)
		}
	}
}
