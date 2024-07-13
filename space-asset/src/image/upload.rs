#![cfg(feature = "runtime")]
use crate::image::ArchivedImage2DDisk;
use crate::uploader::{UploadError, Uploader, ValidatedFrom};
use vulkano::buffer::Buffer as VBuffer;
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, CopyBufferToImageInfo, RecordingCommandBuffer,
};
use vulkano::image::view::ImageView;
use vulkano::image::Image as VImage;
use vulkano::image::{ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::GpuFuture;
use vulkano::{DeviceSize, Validated};
use vulkano_bindless::descriptor::RC;
use vulkano_bindless::spirv_std::image::Image2d;
use vulkano_bindless_shaders::descriptor::Desc;

impl<const DATA_TYPE: u32> ArchivedImage2DDisk<DATA_TYPE> {
	pub async fn upload(&self, uploader: &Uploader) -> Result<Desc<RC, Image2d>, Validated<UploadError>> {
		let metadata = self.metadata.deserialize();

		let upload_buffer = {
			profiling::scope!("image decode to host buffer");
			let upload_buffer = VBuffer::new_slice::<u8>(
				uploader.memory_allocator.clone(),
				BufferCreateInfo {
					usage: BufferUsage::TRANSFER_SRC,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				metadata.decompressed_bytes() as DeviceSize,
			)
			.map_err(UploadError::from_validated)?;
			self.decode_into(&mut *upload_buffer.write().unwrap())
				.map_err(UploadError::from_validated)?;
			upload_buffer
		};

		let perm_image;
		{
			profiling::scope!("image copy cmd");
			perm_image = VImage::new(
				uploader.memory_allocator.clone(),
				ImageCreateInfo {
					image_type: ImageType::Dim2d,
					format: metadata.vulkano_format(),
					extent: [metadata.size.width, metadata.size.height, 1],
					usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
					..ImageCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
					..AllocationCreateInfo::default()
				},
			)
			.map_err(UploadError::from_validated)?;

			let cmd = {
				let mut cmd = RecordingCommandBuffer::new(
					uploader.cmd_allocator.clone(),
					uploader.transfer_queue.queue_family_index(),
					CommandBufferLevel::Primary,
					CommandBufferBeginInfo {
						usage: CommandBufferUsage::OneTimeSubmit,
						..CommandBufferBeginInfo::default()
					},
				)
				.map_err(UploadError::from_validated)?;
				cmd.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(upload_buffer, perm_image.clone()))
					.map_err(UploadError::from_validated)?;
				cmd.end().map_err(UploadError::from_validated)?
			};
			cmd.execute(uploader.transfer_queue.clone())
				.map_err(UploadError::from_validated)?
				.then_signal_fence_and_flush()
				.map_err(UploadError::from_validated)?
		}
		.await
		.map_err(UploadError::from_validated)?;

		Ok(uploader
			.bindless
			.image()
			.alloc_slot_2d(ImageView::new_default(perm_image).map_err(UploadError::from_validated)?)
			.into())
	}
}
