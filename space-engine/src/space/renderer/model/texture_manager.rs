use crate::space::Init;
use image::{DynamicImage, ImageError};
use std::sync::Arc;
use vulkano::buffer::{BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::CommandBufferLevel::Primary;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferUsage, CopyBufferToImageInfo, RecordingCommandBuffer,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::{GpuFuture, Sharing};
use vulkano::DeviceSize;
use vulkano_bindless::descriptor::buffer::Buffer;
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::spirv_std::image::Image2d;

pub struct TextureManager {
	pub init: Arc<Init>,
}

impl TextureManager {
	pub fn new(init: &Arc<Init>) -> Arc<Self> {
		let init = init.clone();
		Arc::new(Self { init })
	}

	pub async fn upload_texture_from_memory(
		&self,
		usage: ImageUsage,
		image_data: &[u8],
	) -> Result<RCDesc<Image2d>, ImageError> {
		Ok(self.upload_texture(usage, image::load_from_memory(image_data)?).await)
	}

	pub async fn upload_texture(&self, usage: ImageUsage, image_data: DynamicImage) -> RCDesc<Image2d> {
		let init = &self.init;
		let image_data = image_data.into_rgba8();
		let (width, height) = image_data.dimensions();

		let copy_buffer = vulkano::buffer::Buffer::new_slice(
			init.memory_allocator.clone(),
			BufferCreateInfo {
				usage: BufferUsage::TRANSFER_SRC,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..AllocationCreateInfo::default()
			},
			image_data.len() as DeviceSize,
		)
		.unwrap();
		copy_buffer.write().unwrap().copy_from_slice(&image_data);

		let image = Image::new(
			init.memory_allocator.clone(),
			ImageCreateInfo {
				image_type: ImageType::Dim2d,
				format: Format::R8G8B8A8_SRGB,
				extent: [width, height, 1],
				usage: ImageUsage::TRANSFER_DST | usage,
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

		self.init.bindless.image.alloc_slot_2d(image_view)
	}

	pub fn upload_buffer<T, ITER>(&self, usage: BufferUsage, data: ITER) -> RCDesc<Buffer<[T]>>
	where
		T: BufferContents,
		ITER: IntoIterator<Item = T>,
		ITER::IntoIter: ExactSizeIterator,
	{
		self.init
			.bindless
			.buffer
			.alloc_from_iter(
				self.init.memory_allocator.clone(),
				BufferCreateInfo {
					usage,
					sharing: Sharing::Exclusive,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				data,
			)
			.unwrap()
	}
}
