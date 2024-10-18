use crate::backend::range_set::{range_to_descriptor_index, DescriptorIndexRangeSet};
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::buffer_metadata_cpu::{BackingRefsError, StrongMetadataCpu};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::{AnyRCDesc, Bindless, RCDescExt};
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct, Metadata};
use rust_gpu_bindless_shaders::descriptor::{Buffer, BINDING_BUFFER};
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::{AllocateBufferError, BufferCreateInfo, Subbuffer};
use vulkano::buffer::{Buffer as VBuffer, BufferContents as VBufferContents};
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, InvalidateDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::shader::ShaderStages;
use vulkano::{DeviceSize, Validated};

impl<T: BufferContent + ?Sized> DescContentCpu for Buffer<T>
where
	T::Transfer: VBufferContents,
{
	type DescTable = BufferTable;
	type VulkanType = Subbuffer<T::Transfer>;

	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType {
		slot.try_deref::<BufferInterface>().unwrap().buffer.reinterpret_ref()
	}
}

impl DescTable for BufferTable {
	type Slot = BufferSlot;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_storage_buffers
			.unwrap()
	}

	fn layout_binding(
		stages: ShaderStages,
		count: DescriptorCounts,
		out: &mut BTreeMap<u32, DescriptorSetLayoutBinding>,
	) {
		out.insert(
			BINDING_BUFFER,
			DescriptorSetLayoutBinding {
				binding_flags: Self::BINDING_FLAGS,
				descriptor_count: count.buffers,
				stages,
				..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer)
			},
		)
		.ok_or(())
		.unwrap_err();
	}
}

pub struct BufferSlot {
	buffer: Subbuffer<[u8]>,
	_strong_refs: StrongBackingRefs,
}

pub struct BufferTable {
	table: Arc<Table<BufferInterface>>,
}

impl BufferTable {
	pub fn new(table_sync: &Arc<TableSync>, descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			table: table_sync.register(count, BufferInterface { descriptor_set }).unwrap(),
		}
	}
}

pub struct BufferTableAccess<'a>(pub &'a Arc<Bindless>);

impl<'a> Deref for BufferTableAccess<'a> {
	type Target = BufferTable;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.buffer
	}
}

pub enum AllocFromError {
	AllocateBufferError(AllocateBufferError),
	BackingRefsError(BackingRefsError),
}

impl AllocFromError {
	fn from_backing_refs(value: BackingRefsError) -> Validated<Self> {
		Validated::Error(Self::BackingRefsError(value))
	}
	fn from_validated_alloc(value: Validated<AllocateBufferError>) -> Validated<Self> {
		match value {
			Validated::Error(e) => Validated::Error(Self::AllocateBufferError(e)),
			Validated::ValidationError(v) => Validated::ValidationError(v),
		}
	}
}

impl Display for AllocFromError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			AllocFromError::AllocateBufferError(e) => e.fmt(f),
			AllocFromError::BackingRefsError(e) => e.fmt(f),
		}
	}
}

impl<'a> BufferTableAccess<'a> {
	#[inline]
	pub fn alloc_slot<T: BufferContent + ?Sized>(
		&self,
		buffer: Subbuffer<T::Transfer>,
		strong_refs: StrongBackingRefs,
	) -> RCDesc<Buffer<T>>
	where
		T::Transfer: VBufferContents,
	{
		unsafe {
			RCDesc::new(
				self.table
					.alloc_slot(BufferSlot {
						buffer: buffer.into_bytes(),
						_strong_refs: strong_refs,
					})
					.map_err(|a| format!("BufferTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub fn alloc_from_data<T: BufferStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocFromError>> {
		unsafe {
			let mut meta = StrongMetadataCpu::new(self.0, Metadata);
			let buffer = VBuffer::from_data(allocator, create_info, allocation_info, T::write_cpu(data, &mut meta))
				.map_err(AllocFromError::from_validated_alloc)?;
			Ok(self.alloc_slot(
				buffer,
				meta.into_backing_refs().map_err(AllocFromError::from_backing_refs)?,
			))
		}
	}

	pub fn alloc_from_iter<T: BufferStruct, I>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		iter: I,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocFromError>>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		unsafe {
			let mut meta = StrongMetadataCpu::new(self.0, Metadata);
			let iter = iter.into_iter().map(|i| T::write_cpu(i, &mut meta));
			let buffer = VBuffer::from_iter(allocator, create_info, allocation_info, iter)
				.map_err(AllocFromError::from_validated_alloc)?;
			Ok(self.alloc_slot(
				buffer,
				meta.into_backing_refs().map_err(AllocFromError::from_backing_refs)?,
			))
		}
	}

	pub fn alloc_sized<T: BufferStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_sized::<T::Transfer>(allocator, create_info, allocation_info)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}

	pub fn alloc_slice<T: BufferStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_slice::<T::Transfer>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}

	pub fn alloc_unsized<T: BufferContent + ?Sized>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>>
	where
		T::Transfer: VBufferContents,
	{
		let buffer = VBuffer::new_unsized::<T::Transfer>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}

	pub(crate) fn flush_descriptors(
		&self,
		delay_drop: &mut Vec<RcTableSlot>,
		mut writes: impl FnMut(WriteDescriptorSet),
	) {
		let flush_queue = self.table.drain_flush_queue();
		let mut set = DescriptorIndexRangeSet::new();
		delay_drop.reserve(flush_queue.size_hint().0);
		for x in flush_queue {
			set.insert(x.id().index());
			delay_drop.push(x);
		}
		for range in set.iter_ranges() {
			writes(WriteDescriptorSet::buffer_array(
				BINDING_BUFFER,
				range.start.to_u32(),
				range_to_descriptor_index(range)
					.map(|index| unsafe { self.table.get_slot_unchecked(index).buffer.clone() }),
			));
		}
	}
}

pub struct BufferInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl TableInterface for BufferInterface {
	type Slot = BufferSlot;

	fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
		for x in indices.iter_ranges() {
			self.descriptor_set
				.invalidate(&[InvalidateDescriptorSet::invalidate_array(
					BINDING_BUFFER,
					x.start.to_u32(),
					(x.end - x.start) as u32,
				)])
				.unwrap();
		}
	}

	fn flush(&self, _flush_queue: DrainFlushQueue<'_, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}

/// Stores [`RC`] to various resources, to which [`StrongDesc`] contained in some resource may refer to.
#[derive(Clone, Default)]
pub struct StrongBackingRefs(pub SmallVec<[AnyRCDesc; 5]>);
