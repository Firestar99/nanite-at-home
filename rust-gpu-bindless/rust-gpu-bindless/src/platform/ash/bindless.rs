use crate::backend::range_set::{DescriptorIndexIterator, DescriptorIndexRangeSet};
use crate::backend::table::DrainFlushQueue;
use crate::descriptor::mutable::MutDesc;
use crate::descriptor::{
	BindlessCreateInfo, BufferInterface, BufferSlot, BufferTableAccess, DescriptorCounts, ImageInterface, RCDesc,
	Sampler, SamplerInterface, SamplerTableAccess, StrongBackingRefs,
};
use crate::platform::ash::Ash;
use crate::platform::BindlessPlatform;
use ash::prelude::VkResult;
use ash::vk::{
	BufferUsageFlags, DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool, DescriptorPoolCreateFlags,
	DescriptorPoolCreateInfo, DescriptorPoolSize, DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout,
	DescriptorSetLayoutCreateFlags, DescriptorSetLayoutCreateInfo, DescriptorType, DeviceSize, ImageLayout,
	ImageUsageFlags, SamplerCreateInfo, WriteDescriptorSet,
};
use ash::vk::{PhysicalDeviceProperties2, PhysicalDeviceVulkan12Properties};
use gpu_allocator::vulkan::{AllocationCreateDesc, AllocationScheme};
use gpu_allocator::{AllocationError, MemoryLocation};
use rangemap::RangeSet;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{
	Buffer, BINDING_BUFFER, BINDING_SAMPLED_IMAGE, BINDING_SAMPLER, BINDING_STORAGE_IMAGE,
};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::mem::size_of;
use std::ops::Deref;
use std::sync::Arc;

unsafe impl BindlessPlatform for Ash {
	unsafe fn update_after_bind_descriptor_limits(ci: &Arc<BindlessCreateInfo<Self>>) -> DescriptorCounts {
		let mut vulkan12properties = PhysicalDeviceVulkan12Properties::default();
		let mut properties2 = PhysicalDeviceProperties2::default().push_next(&mut vulkan12properties);
		ci.instance
			.get_physical_device_properties2(ci.physical_device, &mut properties2);
		DescriptorCounts {
			buffers: vulkan12properties.max_descriptor_set_update_after_bind_storage_buffers,
			image: u32::min(
				vulkan12properties.max_per_stage_descriptor_update_after_bind_storage_images,
				vulkan12properties.max_descriptor_set_update_after_bind_sampled_images,
			),
			samplers: vulkan12properties.max_descriptor_set_update_after_bind_samplers,
		}
	}

	unsafe fn create_descriptor_set(ci: &Arc<BindlessCreateInfo<Self>>) -> Self::BindlessDescriptorSet {
		let bindings = [
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_BUFFER)
				.descriptor_type(DescriptorType::STORAGE_BUFFER)
				.descriptor_count(ci.counts.buffers)
				.stage_flags(ci.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_STORAGE_IMAGE)
				.descriptor_type(DescriptorType::STORAGE_IMAGE)
				.descriptor_count(ci.counts.image)
				.stage_flags(ci.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_SAMPLED_IMAGE)
				.descriptor_type(DescriptorType::SAMPLED_IMAGE)
				.descriptor_count(ci.counts.image)
				.stage_flags(ci.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_SAMPLER)
				.descriptor_type(DescriptorType::SAMPLER)
				.descriptor_count(ci.counts.samplers)
				.stage_flags(ci.shader_stages),
		];

		let layout = ci
			.device
			.create_descriptor_set_layout(
				&DescriptorSetLayoutCreateInfo::default()
					.flags(DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
					.bindings(&bindings),
				None,
			)
			.unwrap();

		let pool = ci
			.device
			.create_descriptor_pool(
				&DescriptorPoolCreateInfo::default()
					.flags(DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
					.pool_sizes(&bindings.map(|b| {
						DescriptorPoolSize::default()
							.ty(b.descriptor_type)
							.descriptor_count(b.descriptor_count)
					})),
				None,
			)
			.unwrap();

		let set = ci
			.device
			.allocate_descriptor_sets(
				&DescriptorSetAllocateInfo::default()
					.descriptor_pool(pool)
					.set_layouts(&[layout]),
			)
			.unwrap()
			.into_iter()
			.next()
			.unwrap();

		AshBindlessDescriptorSet { layout, pool, set }
	}

	unsafe fn update_descriptor_set(
		ci: &Arc<BindlessCreateInfo<Self>>,
		set: &Self::BindlessDescriptorSet,
		mut buffers: DrainFlushQueue<BufferInterface<Self>>,
		mut images: DrainFlushQueue<ImageInterface<Self>>,
		mut samplers: DrainFlushQueue<SamplerInterface<Self>>,
	) {
		let buffers = buffers.into_range_set();
		let buffer_infos = buffers
			.iter()
			.map(|(_, buffer)| {
				DescriptorBufferInfo::default()
					.buffer(buffer.buffer)
					.offset(0)
					.range(buffer.size)
			})
			.collect::<Vec<_>>();
		let mut buffer_info_index = 0;
		let buffers = buffers.iter_ranges().map(|(range, _)| {
			let count = range.end.to_usize() - range.start.to_usize();
			WriteDescriptorSet::default()
				.dst_set(set.set)
				.dst_binding(BINDING_BUFFER)
				.descriptor_type(DescriptorType::STORAGE_BUFFER)
				.dst_array_element(range.start.to_u32())
				.descriptor_count(count as u32)
				.buffer_info({
					let buffer_info_start = buffer_info_index;
					buffer_info_index += count;
					&buffer_infos[buffer_info_start..buffer_info_start + count]
				})
		});

		let (image_table, images) = images.into_inner();
		let mut storage_images = DescriptorIndexRangeSet::new(image_table, RangeSet::new());
		let mut sampled_images = DescriptorIndexRangeSet::new(image_table, RangeSet::new());
		for image_id in images {
			let image = unsafe { image_table.get_slot_unchecked(image_id) };
			if image.usage.contains(ImageUsageFlags::STORAGE) {
				storage_images.insert(image_id);
			}
			if image.usage.contains(ImageUsageFlags::SAMPLED) {
				sampled_images.insert(image_id);
			}
		}

		let storage_image_infos = storage_images
			.iter()
			.map(|(_, storage_image)| {
				DescriptorImageInfo::default()
					.image_view(storage_image.imageview)
					.image_layout(ImageLayout::GENERAL)
			})
			.collect::<Vec<_>>();
		let mut storage_image_info_index = 0;
		let storage_images = storage_images.iter_ranges().map(|(range, _)| {
			let count = range.end.to_usize() - range.start.to_usize();
			WriteDescriptorSet::default()
				.dst_set(set.set)
				.dst_binding(BINDING_STORAGE_IMAGE)
				.descriptor_type(DescriptorType::STORAGE_IMAGE)
				.dst_array_element(range.start.to_u32())
				.descriptor_count(count as u32)
				.image_info({
					let storage_image_info_start = storage_image_info_index;
					storage_image_info_index += count;
					&storage_image_infos[storage_image_info_start..storage_image_info_start + count]
				})
		});

		let sampled_image_infos = sampled_images
			.iter()
			.map(|(_, sampled_image)| {
				DescriptorImageInfo::default()
					.image_view(sampled_image.imageview)
					.image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
			})
			.collect::<Vec<_>>();
		let mut sampled_image_info_index = 0;
		let sampled_images = sampled_images.iter_ranges().map(|(range, _)| {
			let count = range.end.to_usize() - range.start.to_usize();
			WriteDescriptorSet::default()
				.dst_set(set.set)
				.dst_binding(BINDING_SAMPLED_IMAGE)
				.descriptor_type(DescriptorType::SAMPLED_IMAGE)
				.dst_array_element(range.start.to_u32())
				.descriptor_count(count as u32)
				.image_info({
					let sampled_image_info_start = sampled_image_info_index;
					sampled_image_info_index += count;
					&sampled_image_infos[sampled_image_info_start..sampled_image_info_start + count]
				})
		});

		let samplers = samplers.into_range_set();
		let sampler_infos = samplers
			.iter()
			.map(|(_, sampler)| DescriptorImageInfo::default().sampler(*sampler))
			.collect::<Vec<_>>();
		let mut sampler_info_index = 0;
		let samplers = samplers.iter_ranges().map(|(range, _)| {
			let count = range.end.to_usize() - range.start.to_usize();
			WriteDescriptorSet::default()
				.dst_set(set.set)
				.dst_binding(BINDING_SAMPLER)
				.descriptor_type(DescriptorType::SAMPLER)
				.dst_array_element(range.start.to_u32())
				.descriptor_count(count as u32)
				.image_info({
					let sampler_info_start = sampler_info_index;
					sampler_info_index += count;
					&sampler_infos[sampler_info_start..sampler_info_start + count]
				})
		});

		let writes = buffers
			.chain(storage_images)
			.chain(sampled_images)
			.chain(samplers)
			.collect::<Vec<_>>();
		ci.device.update_descriptor_sets(&writes, &[]);
	}

	unsafe fn destroy_descriptor_set(ci: &Arc<BindlessCreateInfo<Self>>, set: Self::BindlessDescriptorSet) {
		// descriptor sets allocated from pool are freed implicitly
		ci.device.destroy_descriptor_pool(set.pool, None);
		ci.device.destroy_descriptor_set_layout(set.layout, None);
	}

	unsafe fn reinterpet_ref_buffer<T: Send + Sync + ?Sized + 'static>(buffer: &Self::Buffer) -> &Self::TypedBuffer<T> {
		buffer
	}

	unsafe fn destroy_buffers<'a>(
		ci: &Arc<BindlessCreateInfo<Self>>,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	) {
		let mut allocator = ci.memory_allocator.lock();
		for (_, buffer) in buffers.into_iter() {
			allocator.free(buffer.memory_allocation.clone()).unwrap();
			ci.device.destroy_buffer(buffer.buffer, None);
		}
	}

	unsafe fn destroy_images<'a>(
		ci: &Arc<BindlessCreateInfo<Self>>,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
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
		ci: &Arc<BindlessCreateInfo<Self>>,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	) {
		for (_, sampler) in samplers.into_iter() {
			ci.device.destroy_sampler(*sampler, None);
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct AshBindlessDescriptorSet {
	pub layout: DescriptorSetLayout,
	pub pool: DescriptorPool,
	pub set: DescriptorSet,
}

impl Deref for AshBindlessDescriptorSet {
	type Target = DescriptorSet;

	fn deref(&self) -> &Self::Target {
		&self.set
	}
}

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
				size: ash_create_info.size,
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
