use crate::renderer::Init;
use image::DynamicImage;
use space_engine_shader::renderer::lod_obj::opaque_model::OpaqueVertex;
use std::sync::Arc;
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
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
use vulkano_bindless::buffer_content::BufferStruct;
use vulkano_bindless::descriptor::buffer::Buffer;
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::spirv_std::image::Image2d;

pub struct OpaqueModelCpu {
	pub vertex_buffer: RCDesc<Buffer<[OpaqueVertex]>>,
	pub index_buffer: RCDesc<Buffer<[u32]>>,
}

impl OpaqueModelCpu {
	pub fn direct<V>(init: &Arc<Init>, vertex_data: V) -> Self
	where
		V: IntoIterator<Item = OpaqueVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let vertex_iter = vertex_data.into_iter();
		Self::indexed(init, 0..vertex_iter.len() as u32, vertex_iter)
	}

	pub fn indexed<I, V>(init: &Arc<Init>, index_data: I, vertex_data: V) -> Self
	where
		I: IntoIterator<Item = u32>,
		I::IntoIter: ExactSizeIterator,
		V: IntoIterator<Item = OpaqueVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let vertex_buffer = Self::upload_buffer(
			init,
			BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
			vertex_data,
		);
		let index_buffer = Self::upload_buffer(
			init,
			BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
			index_data,
		);
		Self {
			vertex_buffer,
			index_buffer,
		}
	}

	fn upload_buffer<T, ITER>(init: &Arc<Init>, usage: BufferUsage, data: ITER) -> RCDesc<Buffer<[T]>>
	where
		T: BufferStruct,
		ITER: IntoIterator<Item = T>,
		ITER::IntoIter: ExactSizeIterator,
	{
		init.bindless
			.buffer()
			.alloc_from_iter(
				init.memory_allocator.clone(),
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

	pub async fn upload_texture(init: &Arc<Init>, usage: ImageUsage, image_data: DynamicImage) -> RCDesc<Image2d> {
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
			// this flush is fine, these resources are not yet in the bindless system
			.then_signal_fence_and_flush()
			.unwrap()
			.await
			.unwrap();

		init.bindless.image().alloc_slot_2d(image_view)
	}
}
