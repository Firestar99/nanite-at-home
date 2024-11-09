use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::mutable::{MutDesc, MutDescExt};
use crate::descriptor::{AnyRCDesc, Bindless, BindlessAllocationScheme, BindlessCreateInfo};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::Buffer;
use smallvec::SmallVec;
use std::mem::size_of;
use std::ops::Deref;
use std::sync::Arc;

impl<T: BufferContent + ?Sized> DescContentCpu for Buffer<T> {
	type DescTable<P: BindlessPlatform> = BufferTable<P>;
	type VulkanType<P: BindlessPlatform> = P::TypedBuffer<T::Transfer>;

	fn deref_table<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::VulkanType<P> {
		unsafe { P::reinterpet_ref_buffer(&slot.try_deref::<BufferInterface<P>>().unwrap().buffer) }
	}
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
	pub _strong_refs: StrongBackingRefs<P>,
}

pub struct BufferTable<P: BindlessPlatform> {
	table: Arc<Table<BufferInterface<P>>>,
}

impl<P: BindlessPlatform> BufferTable<P> {
	pub fn new(
		table_sync: &Arc<TableSync>,
		ci: Arc<BindlessCreateInfo<P>>,
		global_descriptor_set: P::BindlessDescriptorSet,
	) -> Self {
		let count = ci.counts.buffers;
		let interface = BufferInterface {
			ci,
			global_descriptor_set,
		};
		Self {
			table: table_sync.register(count, interface).unwrap(),
		}
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
	pub unsafe fn alloc_slot<T: BufferContent + ?Sized>(&self, buffer: BufferSlot<P>) -> MutDesc<P, Buffer<T>> {
		unsafe {
			MutDesc::new(
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
	) -> Result<MutDesc<P, Buffer<T>>, P::AllocationError> {
		unsafe {
			let size = size_of::<T::Transfer>() as u64;
			let (buffer, memory_allocation) = P::alloc_buffer(&self.0.ci, create_info, size)?;
			Ok(self.alloc_slot(BufferSlot {
				buffer,
				len: 1,
				size,
				usage: create_info.usage,
				memory_allocation,
				_strong_refs: StrongBackingRefs::default(),
			}))
		}
	}

	pub fn alloc_slice<T: BufferStruct>(
		&self,
		create_info: &BindlessBufferCreateInfo,
		len: usize,
	) -> Result<MutDesc<P, Buffer<[T]>>, P::AllocationError> {
		unsafe {
			let size = size_of::<T::Transfer>() as u64 * len as u64;
			let (buffer, memory_allocation) = P::alloc_buffer(&self.0.ci, create_info, size)?;
			Ok(self.alloc_slot(BufferSlot {
				buffer,
				len,
				size,
				usage: create_info.usage,
				memory_allocation,
				_strong_refs: StrongBackingRefs::default(),
			}))
		}
	}

	// TODO implement mapping
	// pub fn alloc_from_data<T: BufferStruct>(
	// 	&self,
	// 	create_info: &BindlessBufferCreateInfo,
	// 	data: T,
	// ) -> Result<MutDesc<P, Buffer<T>>, BindlessAllocationError> {
	// 	let buffer = self.alloc_sized(create_info)?;
	// 	buffer.mapped();
	// 	Ok(buffer)
	// }
	//
	// pub fn alloc_from_iter<T: BufferStruct, I>(
	// 	&self,
	// 	create_info: &BindlessBufferCreateInfo,
	// 	iter: I,
	// ) -> Result<MutDesc<P, Buffer<T>>, BindlessAllocationError>
	// where
	// 	I: IntoIterator<Item = T>,
	// 	I::IntoIter: ExactSizeIterator,
	// {
	// 	let iter = iter.into_iter();
	// 	let buffer = self.alloc_slice(create_info, iter.len() as DeviceSize)?;
	// 	buffer.mapped();
	// 	Ok(buffer)
	// }
}

pub struct BufferInterface<P: BindlessPlatform> {
	ci: Arc<BindlessCreateInfo<P>>,
	global_descriptor_set: P::BindlessDescriptorSet,
}

impl<P: BindlessPlatform> TableInterface for BufferInterface<P> {
	type Slot = BufferSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			P::destroy_buffers(&self.ci, &self.global_descriptor_set, indices);
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
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
