use crate::backing::range_set::{DescriptorIndexIterator, DescriptorIndexRangeSet};
use crate::backing::table::{DrainFlushQueue, SlotAllocationError};
use crate::descriptor::MutDesc;
use crate::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, BufferInterface, BufferSlot, BufferTableAccess, DescriptorCounts, Extent, ImageInterface,
	RCDesc, SampleCount, Sampler, SamplerInterface, SamplerTableAccess,
};
use crate::pipeline::access_lock::AccessLock;
use crate::pipeline::access_type::BufferAccess;
use crate::platform::ash::image_format::FormatExt;
use crate::platform::ash::{Ash, AshMemoryAllocation};
use crate::platform::BindlessPlatform;
use crate::spirv_std::image::{Arrayed, Dimensionality};
use ash::prelude::VkResult;
use ash::vk::{
	BufferUsageFlags, ComponentMapping, DescriptorBindingFlags, DescriptorBufferInfo, DescriptorImageInfo,
	DescriptorPool, DescriptorPoolCreateFlags, DescriptorPoolCreateInfo, DescriptorPoolSize, DescriptorSet,
	DescriptorSetAllocateInfo, DescriptorSetLayout, DescriptorSetLayoutBindingFlagsCreateInfo,
	DescriptorSetLayoutCreateFlags, DescriptorSetLayoutCreateInfo, DescriptorType, Extent3D, ImageLayout,
	ImageSubresourceRange, ImageTiling, ImageUsageFlags, ImageViewCreateInfo, ImageViewType, PipelineLayout,
	PipelineLayoutCreateInfo, PushConstantRange, SamplerCreateInfo, SharingMode, WriteDescriptorSet,
};
use ash::vk::{ImageType as VkImageType, SampleCountFlags};
use ash::vk::{PhysicalDeviceProperties2, PhysicalDeviceVulkan12Properties};
use gpu_allocator::vulkan::{AllocationCreateDesc, AllocationScheme};
use gpu_allocator::{AllocationError, MemoryLocation};
use rangemap::RangeSet;
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{
	BindlessPushConstant, ImageType, MutBuffer, BINDING_BUFFER, BINDING_SAMPLED_IMAGE, BINDING_SAMPLER,
	BINDING_STORAGE_IMAGE,
};
use std::mem::size_of;
use std::ops::Deref;
use std::sync::Weak;
use thiserror::Error;

unsafe impl BindlessPlatform for Ash {
	type BindlessDescriptorSet = AshBindlessDescriptorSet;

	unsafe fn create_platform(create_info: Self::PlatformCreateInfo, bindless_cyclic: &Weak<Bindless<Self>>) -> Self {
		Ash::new(create_info, bindless_cyclic)
	}

	unsafe fn update_after_bind_descriptor_limits(&self) -> DescriptorCounts {
		let mut vulkan12properties = PhysicalDeviceVulkan12Properties::default();
		let mut properties2 = PhysicalDeviceProperties2::default().push_next(&mut vulkan12properties);
		self.instance
			.get_physical_device_properties2(self.physical_device, &mut properties2);
		DescriptorCounts {
			buffers: vulkan12properties.max_descriptor_set_update_after_bind_storage_buffers,
			image: u32::min(
				vulkan12properties.max_per_stage_descriptor_update_after_bind_storage_images,
				vulkan12properties.max_descriptor_set_update_after_bind_sampled_images,
			),
			samplers: vulkan12properties.max_descriptor_set_update_after_bind_samplers,
		}
	}

	unsafe fn create_descriptor_set(&self, counts: DescriptorCounts) -> Self::BindlessDescriptorSet {
		let bindings = [
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_BUFFER)
				.descriptor_type(DescriptorType::STORAGE_BUFFER)
				.descriptor_count(counts.buffers)
				.stage_flags(self.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_STORAGE_IMAGE)
				.descriptor_type(DescriptorType::STORAGE_IMAGE)
				.descriptor_count(counts.image)
				.stage_flags(self.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_SAMPLED_IMAGE)
				.descriptor_type(DescriptorType::SAMPLED_IMAGE)
				.descriptor_count(counts.image)
				.stage_flags(self.shader_stages),
			ash::vk::DescriptorSetLayoutBinding::default()
				.binding(BINDING_SAMPLER)
				.descriptor_type(DescriptorType::SAMPLER)
				.descriptor_count(counts.samplers)
				.stage_flags(self.shader_stages),
		];
		let binding_flags =
			[DescriptorBindingFlags::UPDATE_AFTER_BIND | DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING; 4];
		assert_eq!(bindings.len(), binding_flags.len());

		let set_layout = self
			.device
			.create_descriptor_set_layout(
				&DescriptorSetLayoutCreateInfo::default()
					.flags(DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
					.bindings(&bindings)
					.push_next(&mut DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&binding_flags)),
				None,
			)
			.unwrap();

		let pipeline_layout = self
			.device
			.create_pipeline_layout(
				&PipelineLayoutCreateInfo::default()
					.set_layouts(&[set_layout])
					.push_constant_ranges(&[PushConstantRange {
						offset: 0,
						size: size_of::<BindlessPushConstant>() as u32,
						stage_flags: self.shader_stages,
					}]),
				None,
			)
			.unwrap();

		let pool = self
			.device
			.create_descriptor_pool(
				&DescriptorPoolCreateInfo::default()
					.flags(DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
					.pool_sizes(&bindings.map(|b| {
						DescriptorPoolSize::default()
							.ty(b.descriptor_type)
							.descriptor_count(b.descriptor_count)
					}))
					.max_sets(1),
				None,
			)
			.unwrap();

		let set = self
			.device
			.allocate_descriptor_sets(
				&DescriptorSetAllocateInfo::default()
					.descriptor_pool(pool)
					.set_layouts(&[set_layout]),
			)
			.unwrap()
			.into_iter()
			.next()
			.unwrap();

		AshBindlessDescriptorSet {
			pipeline_layout,
			set_layout,
			pool,
			set,
		}
	}

	unsafe fn bindless_initialized(&self, _bindless: &mut Bindless<Self>) {}

	unsafe fn update_descriptor_set(
		&self,
		set: &Self::BindlessDescriptorSet,
		mut buffers: DrainFlushQueue<BufferInterface<Self>>,
		mut images: DrainFlushQueue<ImageInterface<Self>>,
		mut samplers: DrainFlushQueue<SamplerInterface<Self>>,
	) {
		let (buffer_table, buffers) = buffers.into_inner();
		let mut storage_buffers = DescriptorIndexRangeSet::new(buffer_table, RangeSet::new());
		for buffer_id in buffers {
			let buffer = unsafe { buffer_table.get_slot_unchecked(buffer_id) };
			if buffer.usage.contains(BindlessBufferUsage::STORAGE_BUFFER) {
				storage_buffers.insert(buffer_id);
			}
		}

		let buffer_infos = storage_buffers
			.iter()
			.map(|(_, buffer)| {
				DescriptorBufferInfo::default()
					.buffer(buffer.buffer)
					.offset(0)
					.range(buffer.size)
			})
			.collect::<Vec<_>>();
		let mut buffer_info_index = 0;
		let buffers = storage_buffers.iter_ranges().map(|(range, _)| {
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
			if image.usage.contains(BindlessImageUsage::STORAGE) {
				storage_images.insert(image_id);
			}
			if image.usage.contains(BindlessImageUsage::SAMPLED) {
				sampled_images.insert(image_id);
			}
		}

		let storage_image_infos = storage_images
			.iter()
			.map(|(_, storage_image)| {
				DescriptorImageInfo::default()
					.image_view(*storage_image.imageview.as_ref().unwrap())
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
					.image_view(*sampled_image.imageview.as_ref().unwrap())
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
		self.device.update_descriptor_sets(&writes, &[]);
	}

	unsafe fn destroy_descriptor_set(&self, set: Self::BindlessDescriptorSet) {
		// descriptor sets allocated from pool are freed implicitly
		self.device.destroy_descriptor_pool(set.pool, None);
		self.device.destroy_pipeline_layout(set.pipeline_layout, None);
		self.device.destroy_descriptor_set_layout(set.set_layout, None);
	}

	unsafe fn alloc_buffer(
		&self,
		create_info: &BindlessBufferCreateInfo,
		size: u64,
	) -> Result<(Self::Buffer, Self::MemoryAllocation), Self::AllocationError> {
		let buffer = self.device.create_buffer(
			&ash::vk::BufferCreateInfo::default()
				.usage(create_info.usage.to_ash_buffer_usage_flags())
				.size(size)
				.sharing_mode(SharingMode::EXCLUSIVE),
			None,
		)?;
		let requirements = self.device.get_buffer_memory_requirements(buffer);
		let memory_allocation = self.memory_allocator().allocate(&AllocationCreateDesc {
			requirements,
			name: create_info.name,
			location: create_info.usage.to_gpu_allocator_memory_location(),
			allocation_scheme: create_info.allocation_scheme.to_gpu_allocator_buffer(buffer),
			linear: true,
		})?;
		self.device
			.bind_buffer_memory(buffer, memory_allocation.memory(), memory_allocation.offset())?;
		Ok((buffer, AshMemoryAllocation::new(memory_allocation)))
	}

	unsafe fn alloc_image<T: ImageType>(
		&self,
		create_info: &BindlessImageCreateInfo<T>,
	) -> Result<(Self::Image, Self::ImageView, Self::MemoryAllocation), Self::AllocationError> {
		let image_type = bindless_image_type_to_vk_image_type::<T>().expect("Unsupported ImageType");
		let image_view_type = bindless_image_type_to_vk_image_view_type::<T>().expect("Unsupported ImageType");
		let image = self.device.create_image(
			&ash::vk::ImageCreateInfo::default()
				.flags(ash::vk::ImageCreateFlags::empty())
				.image_type(image_type)
				.format(create_info.format)
				.extent(create_info.extent.into())
				.mip_levels(create_info.mip_levels)
				.array_layers(create_info.array_layers)
				.samples(create_info.samples.to_ash_sample_count_flags())
				.tiling(ImageTiling::OPTIMAL)
				.usage(create_info.usage.to_ash_image_usage_flags())
				.sharing_mode(SharingMode::EXCLUSIVE)
				.initial_layout(ImageLayout::UNDEFINED),
			None,
		)?;
		let requirements = self.device.get_image_memory_requirements(image);
		let memory_allocation = self.memory_allocator().allocate(&AllocationCreateDesc {
			requirements,
			name: create_info.name,
			location: create_info.usage.to_gpu_allocator_memory_location(),
			allocation_scheme: create_info.allocation_scheme.to_gpu_allocator_image(image),
			linear: true,
		})?;
		self.device
			.bind_image_memory(image, memory_allocation.memory(), memory_allocation.offset())?;
		let image_view = if create_info.usage.has_image_view() {
			Some(
				self.device.create_image_view(
					&ImageViewCreateInfo::default()
						.image(image)
						.view_type(image_view_type)
						.format(create_info.format)
						.components(ComponentMapping::default()) // identity
						.subresource_range(ImageSubresourceRange {
							aspect_mask: create_info.format.aspect(),
							base_mip_level: 0,
							level_count: create_info.mip_levels,
							base_array_layer: 0,
							layer_count: create_info.array_layers,
						}),
					None,
				)?,
			)
		} else {
			None
		};
		Ok((image, image_view, AshMemoryAllocation::new(memory_allocation)))
	}

	unsafe fn memory_allocation_to_slab<'a>(
		memory_allocation: &'a Self::MemoryAllocation,
	) -> &'a mut (impl presser::Slab + 'a) {
		memory_allocation.get_mut()
	}

	unsafe fn reinterpet_ref_buffer<T: Send + Sync + ?Sized + 'static>(buffer: &Self::Buffer) -> &Self::TypedBuffer<T> {
		buffer
	}

	unsafe fn destroy_buffers<'a>(
		&self,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	) {
		let mut allocator = self.memory_allocator();
		for (_, buffer) in buffers.into_iter() {
			// Safety: We have exclusive access to BufferSlot in this method. The MemoryAllocation will no longer
			// we accessed by anything nor dropped due to being wrapped in MaybeUninit, so we can safely read and drop
			// it ourselves.
			let allocation = buffer.memory_allocation.take();
			allocator.free(allocation).unwrap();
			self.device.destroy_buffer(buffer.buffer, None);
		}
	}

	unsafe fn destroy_images<'a>(
		&self,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
		images: impl DescriptorIndexIterator<'a, ImageInterface<Self>>,
	) {
		let mut allocator = self.memory_allocator();
		for (_, image) in images.into_iter() {
			// Safety: We have exclusive access to BufferSlot in this method. The MemoryAllocation will no longer
			// we accessed by anything nor dropped due to being wrapped in MaybeUninit, so we can safely read and drop
			// it ourselves.
			let allocation = image.memory_allocation.take();
			allocator.free(allocation).unwrap();
			if let Some(imageview) = image.imageview {
				self.device.destroy_image_view(imageview, None);
			}
			self.device.destroy_image(image.image, None);
		}
	}

	unsafe fn destroy_samplers<'a>(
		&self,
		_global_descriptor_set: &Self::BindlessDescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	) {
		for (_, sampler) in samplers.into_iter() {
			self.device.destroy_sampler(*sampler, None);
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct AshBindlessDescriptorSet {
	pub pipeline_layout: PipelineLayout,
	pub set_layout: DescriptorSetLayout,
	pub pool: DescriptorPool,
	pub set: DescriptorSet,
}

impl Deref for AshBindlessDescriptorSet {
	type Target = DescriptorSet;

	fn deref(&self) -> &Self::Target {
		&self.set
	}
}

#[derive(Debug, Error)]
pub enum AshAllocationError {
	#[error("Vk Error: {0}")]
	Vk(ash::vk::Result),
	#[error("Allocator Error: {0}")]
	Allocation(AllocationError),
	#[error("Slot Error: {0}")]
	Slot(SlotAllocationError),
}

impl From<SlotAllocationError> for AshAllocationError {
	fn from(value: SlotAllocationError) -> Self {
		Self::Slot(value)
	}
}

impl From<ash::vk::Result> for AshAllocationError {
	fn from(value: ash::vk::Result) -> Self {
		Self::Vk(value)
	}
}

impl From<AllocationError> for AshAllocationError {
	fn from(value: AllocationError) -> Self {
		Self::Allocation(value)
	}
}

impl<'a> SamplerTableAccess<'a, Ash> {
	pub fn alloc_ash(
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

impl BindlessAllocationScheme {
	pub fn to_gpu_allocator_buffer(&self, buffer: ash::vk::Buffer) -> AllocationScheme {
		match self {
			BindlessAllocationScheme::Dedicated => AllocationScheme::DedicatedBuffer(buffer),
			BindlessAllocationScheme::AllocatorManaged => AllocationScheme::GpuAllocatorManaged,
		}
	}

	pub fn to_gpu_allocator_image(&self, image: ash::vk::Image) -> AllocationScheme {
		match self {
			BindlessAllocationScheme::Dedicated => AllocationScheme::DedicatedImage(image),
			BindlessAllocationScheme::AllocatorManaged => AllocationScheme::GpuAllocatorManaged,
		}
	}
}

impl BindlessBufferUsage {
	pub fn to_ash_buffer_usage_flags(&self) -> BufferUsageFlags {
		let mut out = BufferUsageFlags::empty();
		if self.contains(BindlessBufferUsage::TRANSFER_SRC) {
			out |= BufferUsageFlags::TRANSFER_SRC;
		}
		if self.contains(BindlessBufferUsage::TRANSFER_DST) {
			out |= BufferUsageFlags::TRANSFER_DST;
		}
		if self.contains(BindlessBufferUsage::UNIFORM_BUFFER) {
			out |= BufferUsageFlags::UNIFORM_BUFFER;
		}
		if self.contains(BindlessBufferUsage::STORAGE_BUFFER) {
			out |= BufferUsageFlags::STORAGE_BUFFER;
		}
		if self.contains(BindlessBufferUsage::INDEX_BUFFER) {
			out |= BufferUsageFlags::INDEX_BUFFER;
		}
		if self.contains(BindlessBufferUsage::VERTEX_BUFFER) {
			out |= BufferUsageFlags::VERTEX_BUFFER;
		}
		if self.contains(BindlessBufferUsage::INDIRECT_BUFFER) {
			out |= BufferUsageFlags::INDIRECT_BUFFER;
		}
		out
	}

	/// prioritizes MAP_WRITE over MAP_READ
	pub fn to_gpu_allocator_memory_location(&self) -> MemoryLocation {
		if self.contains(BindlessBufferUsage::MAP_WRITE) {
			MemoryLocation::CpuToGpu
		} else if self.contains(BindlessBufferUsage::MAP_READ) {
			MemoryLocation::GpuToCpu
		} else {
			MemoryLocation::GpuOnly
		}
	}
}

fn bindless_image_type_to_vk_image_type<T: ImageType>() -> Option<VkImageType> {
	match T::dimensionality() {
		Dimensionality::OneD => Some(VkImageType::TYPE_1D),
		Dimensionality::TwoD => Some(VkImageType::TYPE_2D),
		Dimensionality::ThreeD => Some(VkImageType::TYPE_3D),
		Dimensionality::Cube => Some(VkImageType::TYPE_2D),
		Dimensionality::Rect => Some(VkImageType::TYPE_2D),
		Dimensionality::Buffer => None,
		Dimensionality::SubpassData => None,
	}
}

fn bindless_image_type_to_vk_image_view_type<T: ImageType>() -> Option<ImageViewType> {
	match (T::dimensionality(), T::arrayed()) {
		(Dimensionality::OneD, Arrayed::False) => Some(ImageViewType::TYPE_1D),
		(Dimensionality::OneD, Arrayed::True) => Some(ImageViewType::TYPE_1D_ARRAY),
		(Dimensionality::TwoD, Arrayed::False) => Some(ImageViewType::TYPE_2D),
		(Dimensionality::TwoD, Arrayed::True) => Some(ImageViewType::TYPE_2D_ARRAY),
		(Dimensionality::ThreeD, Arrayed::False) => Some(ImageViewType::TYPE_3D),
		(Dimensionality::ThreeD, Arrayed::True) => None,
		(Dimensionality::Cube, Arrayed::False) => Some(ImageViewType::CUBE),
		(Dimensionality::Cube, Arrayed::True) => Some(ImageViewType::CUBE_ARRAY),
		(Dimensionality::Rect, Arrayed::False) => Some(ImageViewType::TYPE_2D),
		(Dimensionality::Rect, Arrayed::True) => Some(ImageViewType::TYPE_2D_ARRAY),
		(Dimensionality::Buffer, _) => None,
		(Dimensionality::SubpassData, _) => None,
	}
}

impl SampleCount {
	pub fn to_ash_sample_count_flags(&self) -> SampleCountFlags {
		match self {
			SampleCount::Sample1 => SampleCountFlags::TYPE_1,
			SampleCount::Sample2 => SampleCountFlags::TYPE_2,
			SampleCount::Sample4 => SampleCountFlags::TYPE_4,
			SampleCount::Sample8 => SampleCountFlags::TYPE_8,
			SampleCount::Sample16 => SampleCountFlags::TYPE_16,
			SampleCount::Sample32 => SampleCountFlags::TYPE_32,
			SampleCount::Sample64 => SampleCountFlags::TYPE_64,
		}
	}
}

impl From<Extent> for Extent3D {
	fn from(value: Extent) -> Self {
		Extent3D {
			width: value.width,
			height: value.height,
			depth: value.depth,
		}
	}
}

impl BindlessImageUsage {
	pub fn to_ash_image_usage_flags(&self) -> ImageUsageFlags {
		let mut out = ImageUsageFlags::empty();
		if self.contains(BindlessImageUsage::TRANSFER_SRC) {
			out |= ImageUsageFlags::TRANSFER_SRC;
		}
		if self.contains(BindlessImageUsage::TRANSFER_DST) {
			out |= ImageUsageFlags::TRANSFER_DST;
		}
		if self.contains(BindlessImageUsage::SAMPLED) {
			out |= ImageUsageFlags::SAMPLED;
		}
		if self.contains(BindlessImageUsage::STORAGE) {
			out |= ImageUsageFlags::STORAGE;
		}
		if self.contains(BindlessImageUsage::COLOR_ATTACHMENT) {
			out |= ImageUsageFlags::COLOR_ATTACHMENT;
		}
		if self.contains(BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT) {
			out |= ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
		}
		out
	}

	pub fn to_gpu_allocator_memory_location(&self) -> MemoryLocation {
		MemoryLocation::GpuOnly
	}

	pub fn has_image_view(&self) -> bool {
		self.intersects(
			BindlessImageUsage::SAMPLED
				| BindlessImageUsage::STORAGE
				| BindlessImageUsage::COLOR_ATTACHMENT
				| BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT,
		)
	}
}

impl<'a> BufferTableAccess<'a, Ash> {
	/// Create a new buffer directly from an ash's [`BufferCreateInfo`] and the flattened members of GpuAllocator's
	/// [`AllocationCreateDesc`] to allow for maximum customizability.
	///
	/// # Safety
	/// Size must be sufficient to store `T`. If `T` is a slice, `len` must be its length, otherwise it must be 1.
	/// Returned buffer will be uninitialized.
	pub unsafe fn alloc_ash_unchecked<T: BufferContent + ?Sized>(
		&self,
		ash_create_info: &ash::vk::BufferCreateInfo,
		usage: BindlessBufferUsage,
		location: MemoryLocation,
		allocation_scheme: BindlessAllocationScheme,
		len: usize,
		name: &str,
		prev_access_type: BufferAccess,
	) -> Result<MutDesc<Ash, MutBuffer<T>>, AshAllocationError> {
		unsafe {
			let buffer = self.0.device.create_buffer(&ash_create_info, None)?;
			let requirements = self.0.device.get_buffer_memory_requirements(buffer);
			let memory_allocation = self.0.memory_allocator().allocate(&AllocationCreateDesc {
				requirements,
				name,
				location,
				allocation_scheme: allocation_scheme.to_gpu_allocator_buffer(buffer),
				linear: true,
			})?;
			self.0
				.device
				.bind_buffer_memory(buffer, memory_allocation.memory(), memory_allocation.offset())?;
			Ok(self.alloc_slot(BufferSlot {
				buffer,
				len,
				size: ash_create_info.size,
				usage,
				memory_allocation: AshMemoryAllocation::new(memory_allocation),
				strong_refs: Default::default(),
				access_lock: AccessLock::new(prev_access_type),
			})?)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// we want our [`BindlessBufferUsage`] bits to equal ash's [`BufferUsageFlags`] so the conversion can mostly be
	/// optimized away
	#[test]
	fn test_buffer_usage_to_ash_same_bits() {
		for usage in [
			BindlessBufferUsage::TRANSFER_SRC,
			BindlessBufferUsage::TRANSFER_DST,
			BindlessBufferUsage::UNIFORM_BUFFER,
			BindlessBufferUsage::STORAGE_BUFFER,
			BindlessBufferUsage::INDEX_BUFFER,
			BindlessBufferUsage::VERTEX_BUFFER,
			BindlessBufferUsage::INDIRECT_BUFFER,
		] {
			assert_eq!(
				Some(usage),
				BindlessBufferUsage::from_bits(usage.to_ash_buffer_usage_flags().as_raw() as u64)
			)
		}
	}
}
