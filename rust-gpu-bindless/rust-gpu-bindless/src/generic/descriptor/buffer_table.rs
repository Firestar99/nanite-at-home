use crate::generic::backing::range_set::DescriptorIndexIterator;
use crate::generic::backing::table::{
	DrainFlushQueue, RcTableSlot, SlotAllocationError, Table, TableInterface, TableSync,
};
use crate::generic::descriptor::buffer_metadata_cpu::StrongMetadataCpu;
use crate::generic::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::generic::descriptor::mutdesc::{MutBoxDescExt, MutDesc, MutDescExt};
use crate::generic::descriptor::{
	AnyRCDesc, Bindless, BindlessAllocationScheme, DescContentMutCpu, DescriptorCounts, RCDesc, RCDescExt,
};
use crate::generic::pipeline::{AccessLock, AccessLockError, BufferAccess};
use crate::generic::platform::{BindlessPlatform, PendingExecution};
use bytemuck::Pod;
use parking_lot::Mutex;
use presser::Slab;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct, BufferStructPlain};
use rust_gpu_bindless_shaders::buffer_content::{BufferStructIdentity, Metadata};
use rust_gpu_bindless_shaders::descriptor::{Buffer, MutBuffer};
use smallvec::SmallVec;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use thiserror::Error;

impl<T: BufferContent + ?Sized> DescContentCpu for Buffer<T> {
	type DescTable<P: BindlessPlatform> = BufferTable<P>;
}

impl<T: BufferContent + ?Sized> DescContentCpu for MutBuffer<T> {
	type DescTable<P: BindlessPlatform> = BufferTable<P>;
}

impl<T: BufferContent + ?Sized> DescContentMutCpu for MutBuffer<T> {
	type Shared = Buffer<T>;
	type Access = BufferAccess;
}

impl<P: BindlessPlatform> DescTable<P> for BufferTable<P> {
	type Slot = BufferSlot<P>;

	fn get_slot(slot: &RcTableSlot) -> &Self::Slot {
		&slot.try_deref::<BufferInterface<P>>().unwrap()
	}
}

pub struct BufferSlot<P: BindlessPlatform> {
	pub platform: P::Buffer,
	/// len in T's if this is a slice, otherwise 1
	pub len: usize,
	/// the total size of this buffer in bytes
	pub size: u64,
	pub usage: BindlessBufferUsage,
	pub access_lock: AccessLock<BufferAccess>,
	pub strong_refs: Mutex<StrongBackingRefs<P>>,
	/// This may be replaced with a platform-specific getter, once you can query the name from gpu-allocator to not
	/// unnecessarily duplicate the String (see my PR https://github.com/Traverse-Research/gpu-allocator/pull/257)
	pub debug_name: String,
}

impl<P: BindlessPlatform> Deref for BufferSlot<P> {
	type Target = P::Buffer;

	fn deref(&self) -> &Self::Target {
		&self.platform
	}
}

impl<P: BindlessPlatform> BufferSlot<P> {
	pub fn debug_name(&self) -> &str {
		&self.debug_name
	}
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
	#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
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
	#[inline]
	pub fn is_mappable(&self) -> bool {
		self.intersects(BindlessBufferUsage::MAP_READ | BindlessBufferUsage::MAP_WRITE)
	}

	#[inline]
	pub fn initial_buffer_access(&self) -> BufferAccess {
		if self.is_mappable() {
			BufferAccess::General
		} else {
			BufferAccess::Undefined
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct BindlessBufferCreateInfo<'a> {
	/// Buffer usage specify how you may use a buffer. Missing flags are only validated during runtime.
	pub usage: BindlessBufferUsage,
	/// Determines how this allocation should be managed.
	pub allocation_scheme: BindlessAllocationScheme,
	/// Name of the buffer, for tracking and debugging purposes
	pub name: &'a str,
}

impl<'a> Default for BindlessBufferCreateInfo<'a> {
	fn default() -> Self {
		Self {
			usage: BindlessBufferUsage::default(),
			allocation_scheme: BindlessAllocationScheme::default(),
			name: "",
		}
	}
}

impl<'a> BindlessBufferCreateInfo<'a> {
	#[inline]
	pub fn validate<P: BindlessPlatform>(&self) -> Result<(), BufferAllocationError<P>> {
		if self.usage.is_empty() {
			Err(BufferAllocationError::NoUsageDeclared {
				name: self.name.to_string(),
			})
		} else {
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum BufferAllocationError<P: BindlessPlatform> {
	#[error("Platform Error: {0}")]
	Platform(#[source] P::AllocationError),
	#[error("Slot Allocation Error: {0}")]
	Slot(#[from] SlotAllocationError),
	#[error("Buffer {name} must have at least one usage must be declared")]
	NoUsageDeclared { name: String },
}

impl<P: BindlessPlatform> Debug for BufferAllocationError<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}

impl<'a, P: BindlessPlatform> BufferTableAccess<'a, P> {
	/// Allocates a new slot for the supplied buffer
	///
	/// # Safety
	/// The Buffer's device must be the same as the bindless device. Ownership of the buffer is transferred to this
	/// table. You may not access or drop it afterward, except by going though the returned `MutDesc`.
	/// The generic T must match the contents of the Buffer and the size of the buffer must not be smaller than T.
	#[inline]
	pub unsafe fn alloc_slot<T: BufferContent + ?Sized>(
		&self,
		buffer: BufferSlot<P>,
	) -> Result<MutDesc<P, MutBuffer<T>>, SlotAllocationError> {
		unsafe {
			Ok(MutDesc::new(
				self.table.alloc_slot(buffer)?,
				PendingExecution::<P>::new_completed(),
			))
		}
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, BufferInterface<P>> {
		self.table.drain_flush_queue()
	}

	pub fn alloc_sized<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
	) -> Result<MutDesc<P, MutBuffer<T>>, BufferAllocationError<P>> {
		unsafe {
			create_info.validate()?;
			let size = size_of::<T::Transfer>() as u64;
			let buffer = self
				.0
				.platform
				.alloc_buffer(create_info, size)
				.map_err(Into::<BufferAllocationError<P>>::into)?;
			Ok(self.alloc_slot(BufferSlot {
				platform: buffer,
				len: 1,
				size,
				usage: create_info.usage,
				strong_refs: Default::default(),
				access_lock: AccessLock::new(create_info.usage.initial_buffer_access()),
				debug_name: create_info.name.to_string(),
			})?)
		}
	}

	pub fn alloc_slice<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		len: usize,
	) -> Result<MutDesc<P, MutBuffer<[T]>>, BufferAllocationError<P>> {
		unsafe {
			create_info.validate()?;
			let size = size_of::<T::Transfer>() as u64 * len as u64;
			let buffer = self
				.0
				.platform
				.alloc_buffer(create_info, size)
				.map_err(Into::<BufferAllocationError<P>>::into)?;
			Ok(self.alloc_slot(BufferSlot {
				platform: buffer,
				len,
				size,
				usage: create_info.usage,
				strong_refs: Default::default(),
				access_lock: AccessLock::new(create_info.usage.initial_buffer_access()),
				debug_name: create_info.name.to_string(),
			})?)
		}
	}

	pub fn alloc_from_data<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		data: T,
	) -> Result<MutDesc<P, MutBuffer<T>>, BufferAllocationError<P>> {
		let buffer = self.alloc_sized(create_info)?;
		buffer.mapped_immediate().unwrap().write_data(data);
		Ok(buffer)
	}

	pub fn alloc_shared_from_data<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		data: T,
	) -> Result<RCDesc<P, Buffer<T>>, BufferAllocationError<P>> {
		unsafe { Ok(self.alloc_from_data(create_info, data)?.into_shared_unchecked()) }
	}

	pub fn alloc_from_iter<T: BufferStruct, I>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		iter: I,
	) -> Result<MutDesc<P, MutBuffer<[T]>>, BufferAllocationError<P>>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let iter = iter.into_iter();
		let buffer = self.alloc_slice(create_info, iter.len())?;
		buffer.mapped_immediate().unwrap().overwrite_from_iter_exact(iter);
		Ok(buffer)
	}

	pub fn alloc_shared_from_iter<T: BufferStruct, I>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		iter: I,
	) -> Result<RCDesc<P, Buffer<[T]>>, BufferAllocationError<P>>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		unsafe { Ok(self.alloc_from_iter(create_info, iter)?.into_shared_unchecked()) }
	}
}

pub trait DescBufferLenExt<P: BindlessPlatform> {
	fn len(&self) -> usize;
}

impl<P: BindlessPlatform, T: BufferStruct> DescBufferLenExt<P> for RCDesc<P, Buffer<[T]>> {
	fn len(&self) -> usize {
		self.inner_slot().len
	}
}

impl<P: BindlessPlatform, T: BufferStruct> DescBufferLenExt<P> for MutDesc<P, MutBuffer<[T]>> {
	fn len(&self) -> usize {
		self.inner_slot().len
	}
}

impl<P: BindlessPlatform, T: BufferStructPlain, const N: usize> DescBufferLenExt<P> for RCDesc<P, Buffer<[T; N]>>
where
	// see `impl BufferStructPlain for [T; N]`
	T: Default,
	T::Transfer: Pod + Default,
{
	fn len(&self) -> usize {
		N
	}
}

impl<P: BindlessPlatform, T: BufferStructPlain, const N: usize> DescBufferLenExt<P> for MutDesc<P, MutBuffer<[T; N]>>
where
	// see `impl BufferStructPlain for [T; N]`
	T: Default,
	T::Transfer: Pod + Default,
{
	fn len(&self) -> usize {
		N
	}
}

pub trait MutDescBufferExt<P: BindlessPlatform, T: BufferContent + ?Sized> {
	/// Map and access the buffer's contents on the host
	fn mapped(&self) -> impl Future<Output = Result<MappedBuffer<P, T>, MapError>>;

	/// Map and access the buffer's contents on the host
	fn mapped_immediate(&self) -> Result<MappedBuffer<P, T>, MapError>;
}

impl<P: BindlessPlatform, T: BufferContent + ?Sized> MutDescBufferExt<P, T> for MutDesc<P, MutBuffer<T>> {
	async fn mapped(&self) -> Result<MappedBuffer<P, T>, MapError> {
		self.pending_execution().clone().await;
		self.mapped_immediate()
	}

	fn mapped_immediate(&self) -> Result<MappedBuffer<P, T>, MapError> {
		if !self.pending_execution().completed() {
			return Err(MapError::PendingExecution);
		}
		let slot = self.inner_slot();
		if !slot.usage.is_mappable() {
			return Err(MapError::MissingBufferUsage);
		}
		let curr_access = slot.access_lock.try_lock()?;
		if !matches!(curr_access, BufferAccess::General | BufferAccess::HostAccess) {
			slot.access_lock.unlock(curr_access);
			return Err(MapError::IncorrectLayout(curr_access));
		}
		Ok(MappedBuffer {
			table_sync: self.rc_slot().table_sync_arc(),
			slot,
			curr_access,
			_phantom: PhantomData,
		})
	}
}

#[derive(Debug, Error)]
pub enum MapError {
	#[error("An execution is pending on the buffer that has not yet finished")]
	PendingExecution,
	#[error("Buffer is not mappable, BufferUsage is missing `MAP_WRITE` or `MAP_READ` flags")]
	MissingBufferUsage,
	#[error("Buffer must be in BufferAccess::HostAccess or General, but is in {0:?} access")]
	IncorrectLayout(BufferAccess),
	#[error("AccessLockError: {0}")]
	AccessLock(AccessLockError),
}

impl From<AccessLockError> for MapError {
	fn from(value: AccessLockError) -> Self {
		Self::AccessLock(value)
	}
}

pub struct MappedBuffer<'a, P: BindlessPlatform, T: BufferContent + ?Sized> {
	table_sync: Arc<TableSync>,
	slot: &'a BufferSlot<P>,
	curr_access: BufferAccess,
	_phantom: PhantomData<T>,
}

impl<'a, P: BindlessPlatform, T: BufferContent + ?Sized> Drop for MappedBuffer<'a, P, T> {
	fn drop(&mut self) {
		self.slot.access_lock.unlock(self.curr_access);
	}
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
			let slab = P::mapped_buffer_to_slab(&self.slot);
			let record = presser::copy_from_slice_to_offset(&[T::write_cpu(t, &mut meta)], slab, 0).unwrap();
			assert_eq!(record.copy_start_offset, 0, "presser must not add padding");
			*self.slot.strong_refs.lock() = meta.into_backing_refs();
		}
	}

	pub fn read_data(&mut self) -> T {
		// Safety: assume_initialized_as_bytes is safe if this struct has been initialized, and all
		// TODO mapped buffers are initialized (not yet)
		unsafe {
			let slab = P::mapped_buffer_to_slab(&self.slot);
			let t = bytemuck::cast_slice::<u8, T::Transfer>(slab.assume_initialized_as_bytes())[0];
			T::read(t, Metadata {})
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
			let slab = P::mapped_buffer_to_slab(&self.slot);
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
			let slab = P::mapped_buffer_to_slab(&self.slot);
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

// TODO soundness: these methods may allow reading uninitialized buffers, there is no mechanic to ensure they're
//  initialized
impl<'a, P: BindlessPlatform, T: BufferStruct> MappedBuffer<'a, P, [T]> {
	pub fn read_offset(&mut self, index: usize) -> T {
		// Safety: assume_initialized_as_bytes is safe if this struct has been initialized, and all
		// mapped buffers are initialized (not yet)
		unsafe {
			let slab = P::mapped_buffer_to_slab(&self.slot);
			let t = bytemuck::cast_slice::<u8, T::Transfer>(slab.assume_initialized_as_bytes())[index];
			T::read(t, Metadata {})
		}
	}

	pub fn read_iter(&mut self) -> impl ExactSizeIterator<Item = T> + '_ {
		// Safety: assume_initialized_as_bytes is safe if this struct has been initialized, and all
		// mapped buffers are initialized (not yet)
		unsafe {
			let slab = P::mapped_buffer_to_slab(&self.slot);
			let t = bytemuck::cast_slice::<u8, T::Transfer>(slab.assume_initialized_as_bytes());
			t.iter().take(self.len()).copied().map(|t| T::read(t, Metadata {}))
		}
	}
}

impl<'a, P: BindlessPlatform, T: BufferStructIdentity> MappedBuffer<'a, P, T> {
	pub fn deref_mut(&mut self) -> &mut T {
		unsafe {
			let slab = P::mapped_buffer_to_slab(&self.slot);
			&mut bytemuck::cast_slice_mut::<u8, T>(slab.assume_initialized_as_bytes_mut())[0]
		}
	}
}

impl<'a, P: BindlessPlatform, T: BufferStructIdentity> MappedBuffer<'a, P, [T]> {
	pub fn as_mut_slice(&mut self) -> &mut [T] {
		unsafe {
			let slab = P::mapped_buffer_to_slab(&self.slot);
			bytemuck::cast_slice_mut::<u8, T>(slab.assume_initialized_as_bytes_mut())
		}
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
