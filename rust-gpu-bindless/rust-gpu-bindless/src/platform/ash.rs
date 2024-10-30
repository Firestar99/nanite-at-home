use crate::descriptor::{BufferSlot, DescriptorCounts, ImageSlot};
use crate::platform::interface::{BindlessPlatform, Platform};
use ash::vk::{PhysicalDeviceProperties2, PhysicalDeviceVulkan12Properties};

pub struct Ash;

unsafe impl Platform for Ash {
	type Entry = ash::Entry;
	type Instance = ash::Instance;
	type PhysicalDevice = ash::vk::PhysicalDevice;
	type Device = ash::Device;
	type MemoryAllocator = gpu_allocator::vulkan::Allocator;
	type MemoryAllocation = gpu_allocator::vulkan::Allocation;
	type Buffer = ash::vk::Buffer;
	type TypedBuffer<T: Send + Sync + ?Sized + 'static> = Self::Buffer;
	type Image = ash::vk::Image;
	type ImageView = ash::vk::ImageView;
	type Sampler = ash::vk::Sampler;
	type DescriptorSet = ash::vk::DescriptorSet;
}

unsafe impl BindlessPlatform for Ash {
	unsafe fn update_after_bind_descriptor_limits(
		instance: &Self::Instance,
		phy: &Self::PhysicalDevice,
	) -> DescriptorCounts {
		let mut vulkan12properties = PhysicalDeviceVulkan12Properties::default();
		let mut properties2 = PhysicalDeviceProperties2::default().push_next(&mut vulkan12properties);
		instance.get_physical_device_properties2(*phy, &mut properties2);
		DescriptorCounts {
			buffers: vulkan12properties.max_descriptor_set_update_after_bind_storage_buffers,
			image: u32::min(
				vulkan12properties.max_per_stage_descriptor_update_after_bind_storage_images,
				vulkan12properties.max_descriptor_set_update_after_bind_sampled_images,
			),
			samplers: vulkan12properties.max_descriptor_set_update_after_bind_samplers,
		}
	}

	unsafe fn destroy_buffers<'a>(
		device: &Self::Device,
		_global_descriptor_set: &Self::DescriptorSet,
		buffers: impl Iterator<Item = &'a BufferSlot<Self>>,
	) {
		for buffer in buffers {
			device.destroy_buffer(*buffer, None);
		}
	}

	unsafe fn destroy_images<'a>(
		device: &Self::Device,
		_global_descriptor_set: &Self::DescriptorSet,
		images: impl Iterator<Item = ImageSlot<Self>>,
	) {
		for image in images {
			device.destroy_image(*image, None);
		}
	}

	unsafe fn destroy_samplers<'a>(
		device: &Self::Device,
		_global_descriptor_set: &Self::DescriptorSet,
		samplers: impl Iterator<Item = &'a Self::Sampler>,
	) {
		for sampler in samplers {
			device.destroy_sampler(*sampler, None);
		}
	}
}

pub mod extension {
	use crate::descriptor::{
		AllocFromError, BufferSlot, BufferTableAccess, RCDesc, Sampler, SamplerTableAccess, StrongBackingRefs,
	};
	use crate::platform::ash::Ash;
	use ash::prelude::VkResult;
	use ash::vk::{BufferCreateInfo, DeviceSize, SamplerCreateInfo};
	use gpu_allocator::vulkan::{AllocationCreateDesc, AllocationScheme, AllocatorCreateDesc};
	use gpu_allocator::MemoryLocation;
	use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
	use rust_gpu_bindless_shaders::descriptor::Buffer;

	impl<'a> SamplerTableAccess<'a, Ash> {
		pub fn alloc(
			&self,
			device: &ash::Device,
			sampler_create_info: &SamplerCreateInfo,
		) -> VkResult<RCDesc<Sampler>> {
			unsafe {
				let sampler = device.create_sampler(&sampler_create_info, None)?;
				Ok(self.alloc_slot(sampler))
			}
		}
	}

	pub struct BufferAllocationCreateInfo<'a> {
		/// Name of the allocation, for tracking and debugging purposes
		pub name: &'a str,
		/// Location where the memory allocation should be stored
		pub location: MemoryLocation,
		/// Determines how this allocation should be managed.
		pub allocation_scheme: AllocationScheme,
	}

	impl<'a> From<&AllocationCreateDesc<'a>> for BufferAllocationCreateInfo<'a> {
		fn from(value: &AllocationCreateDesc<'a>) -> Self {
			Self {
				name: value.name,
				location: value.location,
				allocation_scheme: value.allocation_scheme,
			}
		}
	}

	impl<'a> BufferTableAccess<'a, Ash> {
		// pub fn alloc_from_data<T: BufferStruct>(
		// 	&self,
		// 	allocator: Arc<dyn MemoryAllocator>,
		// 	create_info: BufferCreateInfo,
		// 	allocation_info: AllocationCreateInfo,
		// 	data: T,
		// ) -> VkResult<RCDesc<Buffer<T>>> {
		// 	unsafe {
		// 		let mut meta = StrongMetadataCpu::new(self.0, Metadata);
		// 		let buffer = VBuffer::from_data(allocator, create_info, allocation_info, T::write_cpu(data, &mut meta))
		// 			.map_err(AllocFromError::from_validated_alloc)?;
		// 		Ok(self.alloc_slot(
		// 			buffer,
		// 			meta.into_backing_refs().map_err(AllocFromError::from_backing_refs)?,
		// 		))
		// 	}
		// }
		//
		// pub fn alloc_from_iter<T: BufferStruct, I>(
		// 	&self,
		// 	allocator: Arc<dyn MemoryAllocator>,
		// 	create_info: BufferCreateInfo,
		// 	allocation_info: AllocationCreateInfo,
		// 	iter: I,
		// ) -> VkResult<RCDesc<Buffer<T>>>
		// where
		// 	I: IntoIterator<Item = T>,
		// 	I::IntoIter: ExactSizeIterator,
		// {
		// 	unsafe {
		// 		let mut meta = StrongMetadataCpu::new(self.0, Metadata);
		// 		let iter = iter.into_iter().map(|i| T::write_cpu(i, &mut meta));
		// 		let buffer = VBuffer::from_iter(allocator, create_info, allocation_info, iter)
		// 			.map_err(AllocFromError::AllocateBufferError)?;
		// 		Ok(self.alloc_slot(
		// 			buffer,
		// 			meta.into_backing_refs().map_err(AllocFromError::BackingRefsError)?,
		// 		))
		// 	}
		// }

		pub unsafe fn create(
			&self,
			create_info: &BufferCreateInfo,
			allocation_info: &BufferAllocationCreateInfo,
			strong_refs: StrongBackingRefs,
		) -> VkResult<RCDesc<Buffer<[u8]>>> {
			unsafe {
				let buffer = self.0.device.create_buffer(create_info, None)?;
				let requirements = self.0.device.get_buffer_memory_requirements(buffer);
				let memory_allocation = self.0.memory_allocator.allocate(&AllocationCreateDesc {
					requirements,
					name: allocation_info.name,
					location: allocation_info.location,
					allocation_scheme: allocation_info.allocation_scheme,
					linear: true,
				})?;
				self.0
					.device
					.bind_buffer_memory(buffer, memory_allocation.memory(), memory_allocation.offset())?;
				Ok(self.alloc_slot(BufferSlot {
					buffer,
					memory_allocation,
					_strong_refs: strong_refs,
				}))
			}
		}

		pub unsafe fn alloc_sized<T: BufferStruct>(
			&self,
			create_info: &BufferCreateInfo,
			allocation_info: &BufferAllocationCreateInfo,
			strong_refs: StrongBackingRefs,
		) -> VkResult<RCDesc<Buffer<T>>> {
		}

		pub fn alloc_slice<T: BufferStruct>(
			&self,
			create_info: &BufferCreateInfo,
			allocation_info: &BufferAllocationCreateInfo,
			len: DeviceSize,
			strong_refs: StrongBackingRefs,
		) -> VkResult<RCDesc<Buffer<T>>> {
			// let buffer = VBuffer::new_slice::<T::Transfer>(allocator, create_info, allocation_info, len)?;
			// Ok(self.alloc_slot(buffer, strong_refs))
		}

		pub fn alloc_unsized<T: BufferContent + ?Sized>(
			&self,
			create_info: &BufferCreateInfo,
			allocation_info: &BufferAllocationCreateInfo,
			len: DeviceSize,
			strong_refs: StrongBackingRefs,
		) -> VkResult<RCDesc<Buffer<T>>> {
			// let buffer = VBuffer::new_unsized::<T::Transfer>(allocator, create_info, allocation_info, len)?;
			// Ok(self.alloc_slot(buffer, strong_refs))
		}
	}
}
