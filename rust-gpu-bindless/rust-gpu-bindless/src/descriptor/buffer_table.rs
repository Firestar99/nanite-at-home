use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::buffer_metadata_cpu::BackingRefsError;
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::{AnyRCDesc, Bindless, DescriptorBinding, RCDescExt, VulkanDescriptorType};
use crate::platform::interface::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, BINDING_BUFFER};
use smallvec::SmallVec;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

impl<T: BufferContent + ?Sized, P: BindlessPlatform> DescContentCpu for Buffer<T> {
	type DescTable = BufferTable<P>;
	type VulkanType = P::TypedBuffer<T::Transfer>;

	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType {
		slot.try_deref::<BufferInterface<P>>().unwrap().buffer.reinterpret_ref()
	}
}

impl<P: BindlessPlatform> DescTable for BufferTable<P> {
	fn layout_binding(count: DescriptorCounts) -> impl Iterator<Item = DescriptorBinding> {
		[DescriptorBinding {
			ty: VulkanDescriptorType::Buffer,
			binding: BINDING_BUFFER,
			count: count.buffers,
		}]
		.into_iter()
	}
}

pub struct BufferSlot<P: BindlessPlatform> {
	pub buffer: P::Buffer,
	pub memory_allocation: P::MemoryAllocation,
	pub _strong_refs: StrongBackingRefs,
}

pub struct BufferTable<P: BindlessPlatform> {
	table: Arc<Table<BufferInterface<P>>>,
}

impl<P: BindlessPlatform> BufferTable<P> {
	pub fn new(
		table_sync: &Arc<TableSync>,
		device: P::Device,
		global_descriptor_set: P::DescriptorSet,
		count: u32,
	) -> Self {
		Self {
			table: table_sync
				.register(
					count,
					BufferInterface {
						device,
						global_descriptor_set,
					},
				)
				.unwrap(),
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
	#[inline]
	pub fn alloc_slot<T: BufferContent + ?Sized>(&self, buffer: BufferSlot<P>) -> RCDesc<Buffer<T>> {
		unsafe {
			RCDesc::new(
				self.table
					.alloc_slot(buffer)
					.map_err(|a| format!("BufferTable: {}", a))
					.unwrap(),
			)
		}
	}

	// pub(crate) fn flush_descriptors(
	// 	&self,
	// 	delay_drop: &mut Vec<RcTableSlot>,
	// 	mut writes: impl FnMut(WriteDescriptorSet),
	// ) {
	// 	let flush_queue = self.table.drain_flush_queue();
	// 	let mut set = DescriptorIndexRangeSet::from();
	// 	delay_drop.reserve(flush_queue.size_hint().0);
	// 	for x in flush_queue {
	// 		set.insert(x.id().index());
	// 		delay_drop.push(x);
	// 	}
	// 	for range in set.iter_ranges() {
	// 		writes(WriteDescriptorSet::buffer_array(
	// 			BINDING_BUFFER,
	// 			range.start.to_u32(),
	// 			range_to_descriptor_index(range)
	// 				.map(|index| unsafe { self.table.get_slot_unchecked(index).buffer.clone() }),
	// 		));
	// 	}
	// }
}

pub struct BufferInterface<P: BindlessPlatform> {
	device: P::Device,
	global_descriptor_set: P::DescriptorSet,
}

impl<P: BindlessPlatform> TableInterface for BufferInterface<P> {
	type Slot = BufferSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			P::destroy_buffers(
				&self.device,
				&self.global_descriptor_set,
				indices.into_iter().map(|(_, s)| &s),
			);
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}

/// Stores [`RC`] to various resources, to which [`StrongDesc`] contained in some resource may refer to.
#[derive(Clone, Default)]
pub struct StrongBackingRefs(pub SmallVec<[AnyRCDesc; 5]>);

pub enum AllocFromError {
	AllocateBufferError(VkError),
	BackingRefsError(BackingRefsError),
}

impl Display for AllocFromError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			AllocFromError::AllocateBufferError(e) => e.fmt(f),
			AllocFromError::BackingRefsError(e) => e.fmt(f),
		}
	}
}
