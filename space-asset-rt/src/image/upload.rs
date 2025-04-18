use crate::uploader::Uploader;
use glam::UVec2;
use rust_gpu_bindless::descriptor::{
	BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, Extent, Format, MutDescBufferExt, RCDesc,
};
use rust_gpu_bindless::pipeline::{MutBufferAccessExt, MutImageAccessExt, TransferRead, TransferWrite};
use rust_gpu_bindless_shaders::descriptor::{Image, Image2d};
use space_asset_disk::image::{ArchivedImage2DDisk, Image2DDisk, Image2DMetadata, ImageType, RuntimeImageCompression};
use std::future::Future;

pub fn upload_image2d_archive<'a, const IMAGE_TYPE: u32>(
	this: &'a ArchivedImage2DDisk<IMAGE_TYPE>,
	name: &str,
	uploader: &'a Uploader,
) -> impl Future<Output = anyhow::Result<RCDesc<Image<Image2d>>>> + 'a {
	upload_image2d(&this.metadata.deserialize(), &this.bytes, name, uploader)
}

pub fn upload_image2d_disk<'a, const IMAGE_TYPE: u32>(
	this: &'a Image2DDisk<IMAGE_TYPE>,
	name: &str,
	uploader: &'a Uploader,
) -> impl Future<Output = anyhow::Result<RCDesc<Image<Image2d>>>> + 'a {
	upload_image2d(&this.metadata, &this.bytes, name, uploader)
}

fn upload_image2d<'a, const IMAGE_TYPE: u32>(
	this: &Image2DMetadata<IMAGE_TYPE>,
	src: &'a [u8],
	name: &str,
	uploader: &'a Uploader,
) -> impl Future<Output = anyhow::Result<RCDesc<Image<Image2d>>>> + 'a {
	let result: anyhow::Result<_> = (|| {
		let bindless = &uploader.bindless;
		let upload_buffer = {
			profiling::scope!("image decode to host buffer");
			let upload_buffer = bindless.buffer().alloc_slice(
				&BindlessBufferCreateInfo {
					usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::TRANSFER_SRC,
					name: &format!("staging image: {name}"),
					allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				},
				this.decompressed_bytes(),
			)?;
			this.decode_into(src, upload_buffer.mapped_immediate()?.as_mut_slice())?;
			upload_buffer
		};

		profiling::scope!("image copy cmd");
		let image = bindless.image().alloc(&BindlessImageCreateInfo {
			format: select_format(this),
			extent: Extent::from(UVec2::from(this.size)),
			usage: BindlessImageUsage::SAMPLED | BindlessImageUsage::TRANSFER_DST,
			name,
			..BindlessImageCreateInfo::default()
		})?;
		Ok(bindless.execute(|cmd| {
			let mut image = image.access::<TransferWrite>(&cmd)?;
			cmd.copy_buffer_to_image(&mut upload_buffer.access::<TransferRead>(&cmd)?, &mut image)?;
			Ok(image.into_shared())
		})?)
	})();
	async { Ok(result?.await) }
}

pub fn select_format<const IMAGE_TYPE: u32>(this: &Image2DMetadata<IMAGE_TYPE>) -> Format {
	match this.runtime_compression() {
		RuntimeImageCompression::None => match this.image_type() {
			ImageType::RValue => Format::R8_UNORM,
			ImageType::RgValue => Format::R8G8_UNORM,
			ImageType::RgbaLinear => Format::R8G8B8A8_UNORM,
			ImageType::RgbaColor => Format::R8G8B8A8_SRGB,
		},
		RuntimeImageCompression::BCn => match this.image_type() {
			ImageType::RValue => Format::BC4_UNORM_BLOCK,
			ImageType::RgValue => Format::BC5_UNORM_BLOCK,
			ImageType::RgbaLinear => Format::BC7_UNORM_BLOCK,
			ImageType::RgbaColor => Format::BC7_SRGB_BLOCK,
		},
	}
}
