use std::marker::PhantomData;
use std::sync::Arc;
use vulkano::buffer::{AllocateBufferError, Buffer, BufferContents, BufferCreateInfo, Subbuffer};

use vulkano::device::Device;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::{DeviceSize, Validated};
use vulkano_bindless_shaders::descriptor::BufferType;

use crate::atomic_slots::AtomicRCSlots;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::table_type::DescTableType;

pub struct DescriptorTable<T: DescTableType> {
	device: Arc<Device>,
	slot_map: Arc<AtomicRCSlots<T::CpuType>>,
	_phantom: PhantomData<T>,
}

pub const SLOTS_FIRST_BLOCK_SIZE: u32 = 128;

impl<T: DescTableType> DescriptorTable<T> {
	pub fn new(device: Arc<Device>) -> Self {
		Self {
			device,
			slot_map: AtomicRCSlots::new(SLOTS_FIRST_BLOCK_SIZE),
			_phantom: PhantomData {},
		}
	}

	pub(crate) fn allocate_slot(&self, cpu_type: T::CpuType) -> RCDesc<T> {
		RCDesc::new(self.slot_map.allocate(cpu_type))
	}
}

impl<T: BufferContents> DescriptorTable<BufferType<T>> {
	pub fn alloc_from_data(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<BufferType<T>>, Validated<AllocateBufferError>> {
		let buffer = Buffer::from_data(allocator, create_info, allocation_info, data)?;
		Ok(self.allocate_slot(buffer.into_bytes()))
	}

	pub fn alloc_from_iter<I>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		iter: I,
	) -> Result<RCDesc<BufferType<T>>, Validated<AllocateBufferError>>
		where
			I: IntoIterator<Item=T>,
			I::IntoIter: ExactSizeIterator,
	{
		let buffer = Buffer::from_iter(allocator, create_info, allocation_info, iter)?;
		Ok(self.allocate_slot(buffer.into_bytes()))
	}

	pub fn alloc_sized(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
	) -> Result<RCDesc<BufferType<T>>, Validated<AllocateBufferError>> {
		let buffer = Buffer::new_sized::<T>(allocator, create_info, allocation_info)?;
		Ok(self.allocate_slot(buffer.into_bytes()))
	}

	pub fn alloc_slice(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<BufferType<[T]>>, Validated<AllocateBufferError>> {
		let buffer: Subbuffer<[T]> = Buffer::new_slice::<T>(allocator, create_info, allocation_info, len)?;
		Ok(self.allocate_slot(buffer.into_bytes()))
	}
}

impl<T: BufferContents + ?Sized> DescriptorTable<BufferType<T>> {
	pub fn alloc_unsized(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<BufferType<T>>, Validated<AllocateBufferError>> {
		let buffer = Buffer::new_unsized::<T>(allocator, create_info, allocation_info, len)?;
		Ok(self.allocate_slot(buffer.into_bytes()))
	}
}
