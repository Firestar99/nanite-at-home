use crate::backend::range_set::DescriptorIndexIterator;
use crate::descriptor::{
	Bindless, BindlessCreateInfo, BufferInterface, DescriptorCounts, ImageInterface, SamplerInterface,
};
use crate::platform::interface::{BindlessPlatform, Platform};
use ash::vk::{PhysicalDeviceProperties2, PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan12Properties};
use gpu_allocator::vulkan::{Allocation, Allocator};
use parking_lot::Mutex;
use static_assertions::assert_impl_all;

pub fn required_features_vk12() -> PhysicalDeviceVulkan12Features<'static> {
	PhysicalDeviceVulkan12Features::default()
		.vulkan_memory_model(true)
		.runtime_descriptor_array(true)
		.descriptor_binding_update_unused_while_pending(true)
		.descriptor_binding_partially_bound(true)
		.descriptor_binding_storage_buffer_update_after_bind(true)
		.descriptor_binding_sampled_image_update_after_bind(true)
		.descriptor_binding_storage_image_update_after_bind(true)
		.descriptor_binding_uniform_buffer_update_after_bind(true)
}

pub struct Ash;
assert_impl_all!(Bindless<Ash>: Send, Sync);

unsafe impl Platform for Ash {
	type Entry = ash::Entry;
	type Instance = ash::Instance;
	type PhysicalDevice = ash::vk::PhysicalDevice;
	type Device = ash::Device;
	type MemoryAllocator = Mutex<Allocator>;
	type MemoryAllocation = Allocation;
	type Buffer = ash::vk::Buffer;
	type TypedBuffer<T: Send + Sync + ?Sized + 'static> = Self::Buffer;
	type Image = ash::vk::Image;
	type ImageView = ash::vk::ImageView;
	type Sampler = ash::vk::Sampler;
	type AllocationError = ();
	type DescriptorSet = ash::vk::DescriptorSet;
}

unsafe impl BindlessPlatform for Ash {
	unsafe fn update_after_bind_descriptor_limits(ci: &BindlessCreateInfo<Self>) -> DescriptorCounts {
		let mut vulkan12properties = PhysicalDeviceVulkan12Properties::default();
		let mut properties2 = PhysicalDeviceProperties2::default().push_next(&mut vulkan12properties);
		ci.instance
			.get_physical_device_properties2(*ci.physical_device, &mut properties2);
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
		ci: &BindlessCreateInfo<Self>,
		_global_descriptor_set: &Self::DescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	) {
		let mut allocator = ci.memory_allocator.lock();
		for (_, buffer) in buffers.into_iter() {
			allocator.free(buffer.memory_allocation.clone()).unwrap();
			ci.device.destroy_buffer(buffer.buffer, None);
		}
	}

	unsafe fn destroy_images<'a>(
		ci: &BindlessCreateInfo<Self>,
		_global_descriptor_set: &Self::DescriptorSet,
		images: impl DescriptorIndexIterator<'a, ImageInterface<Self>>,
	) {
		let mut allocator = ci.memory_allocator.lock();
		for (_, image) in images.into_iter() {
			allocator.free(image.memory_allocation.clone()).unwrap();
			ci.device.destroy_image_view(image.imageview, None);
			ci.device.destroy_image(image.image, None);
		}
	}

	unsafe fn destroy_samplers<'a>(
		ci: &BindlessCreateInfo<Self>,
		_global_descriptor_set: &Self::DescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	) {
		for (_, sampler) in samplers.into_iter() {
			ci.device.destroy_sampler(*sampler, None);
		}
	}
}

pub mod extension {
	use crate::descriptor::mutable::MutDesc;
	use crate::descriptor::{BufferSlot, BufferTableAccess, RCDesc, Sampler, SamplerTableAccess, StrongBackingRefs};
	use crate::platform::ash::Ash;
	use ash::prelude::VkResult;
	use ash::vk::{BufferUsageFlags, DeviceSize, SamplerCreateInfo};
	use gpu_allocator::vulkan::{AllocationCreateDesc, AllocationScheme};
	use gpu_allocator::{AllocationError, MemoryLocation};
	use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
	use rust_gpu_bindless_shaders::descriptor::Buffer;
	use std::error::Error;
	use std::fmt::{Display, Formatter};
	use std::mem::size_of;

	impl<'a> SamplerTableAccess<'a, Ash> {
		pub fn alloc(
			&self,
			device: &ash::Device,
			sampler_create_info: &SamplerCreateInfo,
		) -> VkResult<RCDesc<Ash, Sampler>> {
			unsafe {
				let sampler = device.create_sampler(&sampler_create_info, None)?;
				Ok(self.alloc_slot(sampler))
			}
		}
	}

	// FIXME Mut vs Mapped MutRef, requires custom MemoryLocation and AllocationScheme
	pub struct BindlessBufferCreateInfo<'a> {
		/// allowed buffer usages
		pub usage: BufferUsageFlags,
		/// Name of the allocation, for tracking and debugging purposes
		pub name: &'a str,
		/// Location where the memory allocation should be stored
		pub location: MemoryLocation,
		/// Determines how this allocation should be managed.
		pub allocation_scheme: AllocationScheme,
	}

	#[derive(Debug)]
	pub enum BindlessAllocationError {
		Vk(ash::vk::Result),
		Allocator(AllocationError),
	}

	impl Display for BindlessAllocationError {
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			match self {
				BindlessAllocationError::Vk(e) => Display::fmt(e, f),
				BindlessAllocationError::Allocator(e) => Display::fmt(e, f),
			}
		}
	}

	impl Error for BindlessAllocationError {}

	impl From<ash::vk::Result> for BindlessAllocationError {
		fn from(value: ash::vk::Result) -> Self {
			Self::Vk(value)
		}
	}

	impl From<AllocationError> for BindlessAllocationError {
		fn from(value: AllocationError) -> Self {
			Self::Allocator(value)
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

		/// Create a new buffer directly from an ash's [`BufferCreateInfo`]. Ignores the [`BufferUsageFlags`] from
		/// [`BindlessBufferCreateInfo`].
		///
		/// # Safety
		/// Size must be sufficient to store `T`. If `T` is a slice, `len` must be its length, otherwise it must be 1.
		/// Returned buffer will be uninitialized.
		pub unsafe fn create_ash<T: BufferContent + ?Sized>(
			&self,
			create_info: &BindlessBufferCreateInfo,
			ash_create_info: &ash::vk::BufferCreateInfo,
			len: DeviceSize,
		) -> Result<MutDesc<Ash, Buffer<T>>, BindlessAllocationError> {
			unsafe {
				let buffer = self.0.device.create_buffer(&ash_create_info, None)?;
				let requirements = self.0.device.get_buffer_memory_requirements(buffer);
				let memory_allocation = self.0.memory_allocator.lock().allocate(&AllocationCreateDesc {
					requirements,
					name: create_info.name,
					location: create_info.location,
					allocation_scheme: create_info.allocation_scheme,
					linear: true,
				})?;
				self.0
					.device
					.bind_buffer_memory(buffer, memory_allocation.memory(), memory_allocation.offset())?;
				Ok(self.alloc_slot(BufferSlot {
					buffer,
					len,
					memory_allocation,
					_strong_refs: StrongBackingRefs::default(),
				}))
			}
		}

		pub fn alloc_sized<T: BufferStruct>(
			&self,
			create_info: &BindlessBufferCreateInfo,
		) -> Result<MutDesc<Ash, Buffer<T>>, BindlessAllocationError> {
			unsafe {
				let len = 1;
				self.create_ash(
					create_info,
					&ash::vk::BufferCreateInfo {
						usage: create_info.usage,
						size: size_of::<T>() as DeviceSize * len,
						..Default::default()
					},
					len,
				)
			}
		}

		pub fn alloc_slice<T: BufferStruct>(
			&self,
			create_info: &BindlessBufferCreateInfo,
			len: DeviceSize,
		) -> Result<MutDesc<Ash, Buffer<[T]>>, BindlessAllocationError> {
			unsafe {
				self.create_ash(
					create_info,
					&ash::vk::BufferCreateInfo {
						usage: create_info.usage,
						size: size_of::<T>() as DeviceSize * len,
						..Default::default()
					},
					len,
				)
			}
		}
	}
}
