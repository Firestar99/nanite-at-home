use std::ops::Deref;
use std::sync::Arc;

use vulkano::buffer::Buffer as VBuffer;
use vulkano::buffer::{AllocateBufferError, BufferContents, BufferCreateInfo, Subbuffer};
use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::{DeviceSize, Validated};

use vulkano_bindless_shaders::descriptor::{Buffer, BufferTable};

use crate::atomic_slots::{AtomicRCSlots, AtomicRCSlotsLock, RCSlot};
use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::ResourceTable;
use crate::sync::mpsc::*;

impl<T: BufferContents + ?Sized> DescTypeCpu for Buffer<T> {
	type ResourceTableCpu = BufferTable;
	type CpuType = Subbuffer<T>;

	fn deref_table(slot: &RCSlot<<Self::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> &Self::CpuType {
		slot.deref().reinterpret_ref()
	}

	fn to_table(from: Self::CpuType) -> <Self::ResourceTableCpu as ResourceTableCpu>::SlotType {
		from.into_bytes()
	}
}

impl ResourceTableCpu for BufferTable {
	type SlotType = Subbuffer<[u8]>;
	const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::StorageBuffer;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_storage_buffers
			.unwrap()
	}

	fn write_descriptor_set(
		binding: u32,
		first_array_element: u32,
		elements: impl IntoIterator<Item = Self::SlotType>,
	) -> WriteDescriptorSet {
		WriteDescriptorSet::buffer_array(binding, first_array_element, elements)
	}
}

pub struct BufferResourceTable {
	resource_table: ResourceTable<BufferTable>,
}

impl BufferResourceTable {
	pub fn new(device: Arc<Device>) -> Self {
		Self {
			resource_table: ResourceTable::new(device),
		}
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
}
