use std::ops::DerefMut;
use std::sync::Arc;

use image::{DynamicImage, ImageError};
use parking_lot::Mutex;
use space_engine_common::space::renderer::model::model_vertex::ModelTextureId;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::CommandBufferLevel::Primary;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferUsage, CopyBufferToImageInfo, RecordingCommandBuffer,
};
use vulkano::format::Format;
use vulkano::image::sampler::{Sampler, SamplerCreateInfo};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::GpuFuture;

use crate::space::renderer::model::texture_array_descriptor_set::{
	TextureArrayDescriptorSet, TextureArrayDescriptorSetLayout,
};
use crate::space::Init;

pub struct TextureManager {
	pub init: Arc<Init>,
	pub sampler: Arc<Sampler>,
	pub descriptor_set_layout: TextureArrayDescriptorSetLayout,
	inner: Mutex<Inner>,
}

struct Inner {
	slots: Vec<Arc<ImageView>>,
	descriptor_set_cache: Option<TextureArrayDescriptorSet>,
}

impl TextureManager {
	pub fn new(init: &Arc<Init>) -> Arc<Self> {
		let init = init.clone();
		let sampler = Sampler::new(
			init.device.clone(),
			SamplerCreateInfo {
				..SamplerCreateInfo::simple_repeat_linear()
			},
		)
		.unwrap();
		let descriptor_set_layout = TextureArrayDescriptorSetLayout::new(&init);
		let inner = Mutex::new(Inner {
			slots: Vec::new(),
			descriptor_set_cache: None,
		});
		Arc::new(Self {
			init,
			sampler,
			descriptor_set_layout,
			inner,
		})
	}

	pub async fn upload_texture_from_memory(
		&self,
		image_data: &[u8],
	) -> Result<(Arc<ImageView>, ModelTextureId), ImageError> {
		self.upload_texture(image::load_from_memory(image_data).unwrap()).await
	}

	pub async fn upload_texture(
		&self,
		image_data: DynamicImage,
	) -> Result<(Arc<ImageView>, ModelTextureId), ImageError> {
		let init = &self.init;
		let image_data = image_data.into_rgba8();
		let (width, height) = image_data.dimensions();

		let copy_buffer = Buffer::from_iter(
			init.memory_allocator.clone(),
			BufferCreateInfo {
				usage: BufferUsage::TRANSFER_SRC,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..AllocationCreateInfo::default()
			},
			image_data.into_raw().into_iter(),
		)
		.unwrap();

		let image = Image::new(
			init.memory_allocator.clone(),
			ImageCreateInfo {
				image_type: ImageType::Dim2d,
				format: Format::R8G8B8A8_SRGB,
				extent: [width, height, 1],
				usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
				..ImageCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
				..AllocationCreateInfo::default()
			},
		)
		.unwrap();
		let image_view = ImageView::new_default(image.clone()).unwrap();

		let mut builder = RecordingCommandBuffer::new(
			init.cmd_buffer_allocator.clone(),
			init.queues.client.transfer.queue_family_index(),
			Primary,
			CommandBufferBeginInfo {
				usage: CommandBufferUsage::OneTimeSubmit,
				..CommandBufferBeginInfo::default()
			},
		)
		.unwrap();
		builder
			.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(copy_buffer, image.clone()))
			.unwrap();
		builder
			.end()
			.unwrap()
			.execute(init.queues.client.transfer.clone())
			.unwrap()
			.then_signal_fence_and_flush()
			.unwrap()
			.await
			.unwrap();

		let tex_id;
		{
			let mut inner = self.inner.lock();
			tex_id = ModelTextureId(inner.slots.len() as u32);
			inner.slots.push(image_view.clone());
			inner.descriptor_set_cache = None;
		}
		Ok((image_view, tex_id))
	}

	pub fn create_descriptor_set(&self) -> TextureArrayDescriptorSet {
		let mut guard = self.inner.lock();
		let inner = guard.deref_mut();
		inner
			.descriptor_set_cache
			.get_or_insert_with(|| {
				TextureArrayDescriptorSet::new(&self.init, &self.descriptor_set_layout, &inner.slots)
			})
			.clone()
	}
}
