use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::{FlushUpdates, Lock, ResourceTable};
use crate::rc_slots::{RCSlotsInterface, SlotIndex};
use std::collections::BTreeMap;
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
use vulkano_bindless_shaders::descriptor::BINDING_BUFFER;

impl<T: DescBuffer + ?Sized> DescTypeCpu for Buffer<T>
where
	T::TransferDescBuffer: VBufferContents,
{
	type DescTable = BufferTable;
	type VulkanType = Subbuffer<T::TransferDescBuffer>;

	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType {
		slot.reinterpret_ref()
	}

	fn to_table(from: Self::VulkanType) -> <Self::DescTable as DescTable>::Slot {
		from.into_bytes()
	}
}

impl DescTable for BufferTable {
	type Slot = Subbuffer<[u8]>;
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

	fn lock_table(&self) -> Lock<Self> {
		self.resource_table.lock()
	}
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

	#[inline]
	pub fn alloc_slot<T: DescBuffer + ?Sized>(&self, buffer: Subbuffer<T::TransferDescBuffer>) -> RCDesc<Buffer<T>>
	where
		T::TransferDescBuffer: VBufferContents,
	{
		self.resource_table.alloc_slot(buffer)
	}

	pub fn alloc_from_data<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::from_data(allocator, create_info, allocation_info, unsafe { data.to_transfer() })?;
		Ok(self.alloc_slot(buffer))
	}

	pub fn alloc_from_iter<T: DescStruct, I>(
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
		let iter = iter.into_iter().map(|i| unsafe { T::to_transfer(i) });
		let buffer = VBuffer::from_iter(allocator, create_info, allocation_info, iter)?;
		Ok(self.alloc_slot(buffer))
	}

	pub fn alloc_sized<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_sized::<T::TransferDescStruct>(allocator, create_info, allocation_info)?;
		Ok(self.alloc_slot(buffer))
	}

	pub fn alloc_slice<T: DescStruct>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>> {
		let buffer = VBuffer::new_slice::<T::TransferDescStruct>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer))
	}

	pub fn alloc_unsized<T: DescBuffer + ?Sized>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>>
	where
		T::TransferDescBuffer: VBufferContents,
	{
		let buffer = VBuffer::new_unsized::<T::TransferDescBuffer>(allocator, create_info, allocation_info, len)?;
		Ok(self.alloc_slot(buffer))
	}

	pub(crate) fn flush_updates(&self, mut writes: impl FnMut(WriteDescriptorSet)) -> FlushUpdates<BufferTable> {
		let flush_updates = self.resource_table.flush_updates();
		flush_updates.iter(|first_array_element, buffer| {
			writes(WriteDescriptorSet::buffer_array(
				BINDING_BUFFER,
				first_array_element,
				buffer.drain(..),
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
