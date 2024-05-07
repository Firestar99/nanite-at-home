use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::ResourceTable;
use crate::rc_slots::RCSlot;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::Buffer as VBuffer;
use vulkano::buffer::{AllocateBufferError, BufferContents, BufferCreateInfo, Subbuffer};
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::shader::ShaderStages;
use vulkano::{DeviceSize, Validated};
use vulkano_bindless_shaders::descriptor::buffer::Buffer;
use vulkano_bindless_shaders::descriptor::BINDING_BUFFER;

impl<T: BufferContents + ?Sized> DescTypeCpu for Buffer<T> {
	type DescTable = BufferTable;
	type VulkanType = Subbuffer<T>;

	fn deref_table(slot: &RCSlot<<Self::DescTable as DescTable>::Slot>) -> &Self::VulkanType {
		slot.deref().reinterpret_ref()
	}

	fn to_table(from: Self::VulkanType) -> <Self::DescTable as DescTable>::Slot {
		from.into_bytes()
	}
}

impl DescTable for BufferTable {
	type Slot = Subbuffer<[u8]>;

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

pub struct BufferTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<BufferTable>,
}

impl BufferTable {
	pub fn new(device: Arc<Device>, count: u32) -> Self {
		Self {
			device,
			resource_table: ResourceTable::new(count),
		}
	}

	#[inline]
	pub fn alloc_slot<T: BufferContents + ?Sized>(&self, buffer: Subbuffer<T>) -> RCDesc<Buffer<T>> {
		self.resource_table.alloc_slot(buffer)
	}

	pub fn alloc_from_data<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::from_data(allocator, create_info, allocation_info, data)?;
		Ok(self.resource_table.alloc_slot(buffer))
	}

	pub fn alloc_from_iter<T: BufferContents, I>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		iter: I,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let buffer = VBuffer::from_iter(allocator, create_info, allocation_info, iter)?;
		Ok(self.resource_table.alloc_slot(buffer))
	}

	pub fn alloc_sized<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_sized::<T>(allocator, create_info, allocation_info)?;
		Ok(self.resource_table.alloc_slot(buffer))
	}

	pub fn alloc_slice<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_slice::<T>(allocator, create_info, allocation_info, len)?;
		Ok(self.resource_table.alloc_slot(buffer))
	}

	pub fn alloc_unsized<T: BufferContents + ?Sized>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_unsized::<T>(allocator, create_info, allocation_info, len)?;
		Ok(self.resource_table.alloc_slot(buffer))
	}

	pub(crate) fn flush_updates<const C: usize>(&self, writes: &mut SmallVec<[WriteDescriptorSet; C]>) {
		self.resource_table.flush_updates(|first_array_element, buffer| {
			writes.push(WriteDescriptorSet::buffer_array(
				BINDING_BUFFER,
				first_array_element,
				buffer.drain(..),
			));
		})
	}
}
