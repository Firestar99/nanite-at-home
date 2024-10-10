use crate::uploader::{UploadError, Uploader, ValidatedFrom};
use space_asset_disk::image::{ArchivedImage2DDisk, Image2DDisk, Image2DMetadata, ImageType, RuntimeImageCompression};
use std::future::Future;
use vulkano::buffer::Buffer as VBuffer;
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, CopyBufferToImageInfo, RecordingCommandBuffer,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::Image as VImage;
use vulkano::image::{ImageCreateInfo, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::GpuFuture;
use vulkano::{DeviceSize, Validated};
use vulkano_bindless::descriptor::RC;
use vulkano_bindless::spirv_std::image::Image2d;
use vulkano_bindless_shaders::descriptor::Desc;

pub fn upload_image2d_archive<'a, const IMAGE_TYPE: u32>(
	this: &'a ArchivedImage2DDisk<IMAGE_TYPE>,
	uploader: &'a Uploader,
) -> impl Future<Output = Result<Desc<RC, Image2d>, Validated<UploadError>>> + 'a {
	upload_image2d(&this.metadata.deserialize(), &this.bytes, uploader)
}

pub fn upload_image2d_disk<'a, const IMAGE_TYPE: u32>(
	this: &'a Image2DDisk<IMAGE_TYPE>,
	uploader: &'a Uploader,
) -> impl Future<Output = Result<Desc<RC, Image2d>, Validated<UploadError>>> + 'a {
	upload_image2d(&this.metadata, &this.bytes, uploader)
}

fn upload_image2d<'a, const IMAGE_TYPE: u32>(
	this: &Image2DMetadata<IMAGE_TYPE>,
	src: &'a [u8],
	uploader: &'a Uploader,
) -> impl Future<Output = Result<Desc<RC, Image2d>, Validated<UploadError>>> + 'a {
	let result: Result<_, Validated<UploadError>> = (|| {
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
				this.decompressed_bytes() as DeviceSize,
			)
			.map_err(UploadError::from_validated)?;
			this.decode_into(src, &mut upload_buffer.write().unwrap())
				.map_err(UploadError::from_validated)?;
			upload_buffer
		};

		let perm_image = {
			profiling::scope!("image copy cmd");
			VImage::new(
				uploader.memory_allocator.clone(),
				ImageCreateInfo {
					image_type: vulkano::image::ImageType::Dim2d,
					format: vulkano_format(this),
					extent: [this.size.width, this.size.height, 1],
					usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
					..ImageCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
					..AllocationCreateInfo::default()
				},
			)
			.map_err(UploadError::from_validated)?
		};

		let fence = {
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
		};
		Ok((perm_image, fence))
	})();

	async {
		let (perm_image, fence) = result?;
		fence.await.map_err(UploadError::from_validated)?;
		Ok(uploader
			.bindless
			.image()
			.alloc_slot_2d(ImageView::new_default(perm_image).map_err(UploadError::from_validated)?))
	}
}

pub fn vulkano_format<const IMAGE_TYPE: u32>(this: &Image2DMetadata<IMAGE_TYPE>) -> Format {
	match this.runtime_compression() {
		RuntimeImageCompression::None => match this.image_type() {
			ImageType::R_VALUES => Format::R8_UNORM,
			ImageType::RG_VALUES => Format::R8G8_UNORM,
			ImageType::RGBA_LINEAR => Format::R8G8B8A8_UNORM,
			ImageType::RGBA_COLOR => Format::R8G8B8A8_SRGB,
		},
		RuntimeImageCompression::BCn => match this.image_type() {
			ImageType::R_VALUES => Format::BC4_UNORM_BLOCK,
			ImageType::RG_VALUES => Format::BC5_UNORM_BLOCK,
			ImageType::RGBA_LINEAR => Format::BC7_UNORM_BLOCK,
			ImageType::RGBA_COLOR => Format::BC7_SRGB_BLOCK,
		},
	}
}
