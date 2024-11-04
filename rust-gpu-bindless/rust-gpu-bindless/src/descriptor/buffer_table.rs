use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::mutable::{MutDesc, MutDescExt};
use crate::descriptor::{AnyRCDesc, Bindless, BindlessCreateInfo};
use crate::platform::BindlessPlatform;
use ash::vk::DeviceSize;
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::Buffer;
use smallvec::SmallVec;
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
	pub len: DeviceSize,
	/// the total size of this buffer in bytes
	pub size: DeviceSize,
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
