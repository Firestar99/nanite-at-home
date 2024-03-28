use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::{AllocateBufferError, BufferContents, BufferCreateInfo, Subbuffer};

use vulkano::device::Device;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator};
use vulkano::{DeviceSize, Validated};
use vulkano_bindless_shaders::descriptor::Buffer;

use crate::atomic_slots::{AtomicRCSlots, AtomicRCSlotsLock, RCSlot};
use crate::descriptor::descriptor_cpu_type::DescCpuType;
use crate::descriptor::rc_reference::RCDesc;

use vulkano::buffer::Buffer as VBuffer;
use vulkano::descriptor_set::WriteDescriptorSet;

impl<T: BufferContents + ?Sized> DescCpuType for Buffer<T> {
	type TableType = Subbuffer<[u8]>;
	type CpuType = Subbuffer<T>;

	fn deref_table(slot: &RCSlot<Self::TableType>) -> &Self::CpuType {
		slot.deref().reinterpret_ref()
	}
}

pub struct BufferTable {
	device: Arc<Device>,
	slot_map: Arc<AtomicRCSlots<Subbuffer<[u8]>>>,
}

pub const SLOTS_FIRST_BLOCK_SIZE: u32 = 128;

impl BufferTable {
	pub fn new(device: Arc<Device>) -> Self {
		Self {
			device,
			slot_map: AtomicRCSlots::new(SLOTS_FIRST_BLOCK_SIZE),
		}
	}

	pub fn alloc_slot<T: BufferContents + ?Sized>(&self, buffer: Subbuffer<T>) -> RCDesc<Buffer<T>> {
		RCDesc::new(self.slot_map.allocate(buffer.into_bytes()))
	}

	pub fn alloc_from_data<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		Ok(self.alloc_slot(VBuffer::from_data(allocator, create_info, allocation_info, data)?))
	}

	pub fn alloc_from_iter<T: BufferContents, I>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		iter: I,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>>
		where
			I: IntoIterator<Item=T>,
			I::IntoIter: ExactSizeIterator,
	{
		Ok(self.alloc_slot(VBuffer::from_iter(allocator, create_info, allocation_info, iter)?))
	}

	pub fn alloc_sized<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		Ok(self.alloc_slot(VBuffer::new_sized::<T>(allocator, create_info, allocation_info)?))
	}

	pub fn alloc_slice<T: BufferContents>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<AllocateBufferError>> {
		Ok(self.alloc_slot(VBuffer::new_slice::<T>(allocator, create_info, allocation_info, len)?))
	}

	pub fn alloc_unsized<T: BufferContents + ?Sized>(
		&self,
		allocator: Arc<dyn MemoryAllocator>,
		create_info: BufferCreateInfo,
		allocation_info: AllocationCreateInfo,
		len: DeviceSize,
	) -> Result<RCDesc<Buffer<T>>, Validated<AllocateBufferError>> {
		Ok(self.alloc_slot(VBuffer::new_unsized::<T>(allocator, create_info, allocation_info, len)?))
	}

	pub fn lock(&self) -> BufferTableLock {
		BufferTableLock(self.slot_map.lock())
	}
}

pub struct BufferTableLock(AtomicRCSlotsLock<Subbuffer<[u8]>>);

impl BufferTableLock {
	pub fn unlock(self) {}

	pub fn write(&self) {

		// WriteDescriptorSet::buffer_array(binding, 0, self.0.iter_with(|b|))
	}
}
