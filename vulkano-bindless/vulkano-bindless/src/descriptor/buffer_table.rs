use crate::descriptor::buffer_metadata_cpu::{BackingRefsError, StrongMetadataCpu};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::{AnyRCDesc, RCDesc};
use crate::descriptor::resource_table::{FlushUpdates, ResourceTable, TableEpochGuard};
use crate::descriptor::{Bindless, ImageTable, SamplerTable};
use crate::rc_slot::{RCSlotsInterface, SlotIndex};
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
use vulkano::device::{Device, DeviceOwned};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::shader::ShaderStages;
use vulkano::{DeviceSize, Validated};
use vulkano_bindless_shaders::desc_buffer::{DescBuffer, DescStruct};
use vulkano_bindless_shaders::descriptor::buffer::Buffer;
use vulkano_bindless_shaders::descriptor::descriptor_type::DescEnum;
use vulkano_bindless_shaders::descriptor::metadata::Metadata;
use vulkano_bindless_shaders::descriptor::BINDING_BUFFER;

impl<T: DescBuffer + ?Sized> DescTypeCpu for Buffer<T>
where
	T::TransferDescBuffer: VBufferContents,
{
	type DescTable = BufferTable;
	type VulkanType = Subbuffer<T::TransferDescBuffer>;

	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType {
		slot.buffer.reinterpret_ref()
	}
}

impl DescTable for BufferTable {
	const DESC_ENUM: DescEnum = DescEnum::Buffer;
	type Slot = BufferSlot;
	type RCSlotsInterface = BufferInterface;

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

	fn lock_table(&self) -> TableEpochGuard<Self> {
		self.resource_table.epoch_guard()
	}
}

pub struct BufferSlot {
	buffer: Subbuffer<[u8]>,
	_strong_refs: StrongBackingRefs,
}

pub struct BufferTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<BufferTable>,
}

impl BufferTable {
	pub fn new(descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			device: descriptor_set.device().clone(),
			resource_table: ResourceTable::new(count, BufferInterface { descriptor_set }),
		}
	}
}

pub struct BufferTableAccess<'a>(pub &'a Arc<Bindless>);

impl<'a> Deref for BufferTableAccess<'a> {
	type Target = BufferTable;

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
	pub fn alloc_slot<T: DescBuffer + ?Sized>(
		&self,
		buffer: Subbuffer<T::TransferDescBuffer>,
		strong_refs: StrongBackingRefs,
	) -> RCDesc<Buffer<T>>
	where
		T::TransferDescBuffer: VBufferContents,
	{
		self.resource_table.alloc_slot(BufferSlot {
			buffer: buffer.into_bytes(),
			_strong_refs: strong_refs,
		})
	}

	pub fn alloc_from_data<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocFromError>> {
		unsafe {
			let mut meta = StrongMetadataCpu::new(Metadata);
			let buffer = VBuffer::from_data(allocator, create_info, allocation_info, T::write_cpu(data, &mut meta))
				.map_err(AllocFromError::from_validated_alloc)?;
			Ok(self.alloc_slot(
				buffer,
				meta.into_backing_refs(self.0)
					.map_err(AllocFromError::from_backing_refs)?,
			))
		}
	}

	pub fn alloc_from_iter<T: DescStruct, I>(
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
			let mut meta = StrongMetadataCpu::new(Metadata);
			let iter = iter.into_iter().map(|i| T::write_cpu(i, &mut meta));
			let buffer = VBuffer::from_iter(allocator, create_info, allocation_info, iter)
				.map_err(AllocFromError::from_validated_alloc)?;
			Ok(self.alloc_slot(
				buffer,
				meta.into_backing_refs(self.0)
					.map_err(AllocFromError::from_backing_refs)?,
			))
		}
	}

	pub fn alloc_sized<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_sized::<T::TransferDescStruct>(allocator, create_info, allocation_info)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}

	pub fn alloc_slice<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_slice::<T::TransferDescStruct>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}

	pub fn alloc_unsized<T: DescBuffer + ?Sized>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
		strong_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>>
	where
		T::TransferDescBuffer: VBufferContents,
	{
		let buffer = VBuffer::new_unsized::<T::TransferDescBuffer>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer, strong_refs))
	}
}

impl BufferTable {
	pub(crate) fn flush_updates(&self, mut writes: impl FnMut(WriteDescriptorSet)) -> FlushUpdates<BufferTable> {
		let flush_updates = self.resource_table.flush_updates();
		flush_updates.iter(|first_array_element, buffer| {
			writes(WriteDescriptorSet::buffer_array(
				BINDING_BUFFER,
				first_array_element,
				buffer.iter().map(|slot| slot.buffer.clone()),
			));
		});
		flush_updates
	}
}

pub struct BufferInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl RCSlotsInterface<<BufferTable as DescTable>::Slot> for BufferInterface {
	fn drop_slot(&self, index: SlotIndex, t: <BufferTable as DescTable>::Slot) {
		self.descriptor_set
			.invalidate(&[InvalidateDescriptorSet::invalidate_array(
				BINDING_BUFFER,
				index.0 as u32,
				1,
			)])
			.unwrap();
		drop(t);
	}
}

/// Stores [`AnyRCDesc`] to various resources, to which [`StrongDesc`] contained in some resource may refer to.
#[derive(Clone, Default)]
pub struct StrongBackingRefs {
	pub _buffer: SmallVec<[AnyRCDesc<BufferTable>; 4]>,
	pub _image: SmallVec<[AnyRCDesc<ImageTable>; 4]>,
	pub _sampler: SmallVec<[AnyRCDesc<SamplerTable>; 1]>,
}
