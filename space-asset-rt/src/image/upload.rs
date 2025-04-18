use futures::future::join_all;
use glam::Vec4;
use rayon::prelude::*;
use rkyv::api::high::HighDeserializer;
use rkyv::rancor::Panic;
use rkyv::Deserialize;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, Extent, Format, MutDescBufferExt, RCDesc,
};
use rust_gpu_bindless::pipeline::{MutBufferAccessExt, MutImageAccessExt, TransferRead, TransferWrite};
use rust_gpu_bindless_shaders::descriptor::{Image, Image2d};
use space_asset_disk::image::{
	ArchivedImageStorage, DynImage, ImageDiskTrait, ImageType, RuntimeImageCompression, RuntimeImageMetadata,
	SinglePixelMetadata,
};
use std::future::Future;

pub struct UploadedImages {
	images: Vec<RCDesc<Image<Image2d>>>,
	pub default_white_texture: RCDesc<Image<Image2d>>,
	pub default_normal_texture: RCDesc<Image<Image2d>>,
}

impl UploadedImages {
	pub fn new<'a>(
		bindless: &'a Bindless,
		storage: &'a ArchivedImageStorage,
	) -> impl Future<Output = anyhow::Result<Self>> + 'a {
		let defaults = join_all(
			[
				(Vec4::splat(1.), "default_white_texture"),
				(Vec4::splat(0.5), "default_normal_texture"),
			]
			.iter()
			.map(|(color, name)| {
				let color = color.to_array().map(|f| (f * 255.) as u8);
				upload_image(
					&bindless,
					&SinglePixelMetadata::new_rgba_linear(color).to_image().to_dyn_image(),
					&name,
				)
			}),
		);
		let images = join_all(
			storage
				.images
				.par_iter()
				.map(|i| upload_image(&bindless, &i.0.to_image(), &i.1))
				.collect::<Vec<_>>()
				.into_iter(),
		);
		async {
			let defaults = defaults.await;
			Ok(Self {
				images: images.await.into_iter().collect::<Result<Vec<_>, _>>()?,
				default_white_texture: defaults[0].as_ref().unwrap().clone(),
				default_normal_texture: defaults[1].as_ref().unwrap().clone(),
			})
		}
	}

	pub fn image<I: ImageDiskTrait>(&self, image: I) -> &RCDesc<Image<Image2d>> {
		&self.images[image.id()]
	}

	pub fn archived_image<A, I: ImageDiskTrait>(&self, image: &A) -> &RCDesc<Image<Image2d>>
	where
		A: Deserialize<I, HighDeserializer<Panic>>,
	{
		self.image(rkyv::deserialize(image).unwrap())
	}
}

pub fn upload_image<'a>(
	bindless: &'a Bindless,
	image: &DynImage,
	name: &str,
) -> impl Future<Output = anyhow::Result<RCDesc<Image<Image2d>>>> + 'a {
	let result: anyhow::Result<_> = (|| {
		let meta = image.decoded_metadata();

		let upload_buffer = {
			profiling::scope!("image decode to host buffer");
			let upload_buffer = bindless.buffer().alloc_slice(
				&BindlessBufferCreateInfo {
					usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::TRANSFER_SRC,
					name: &format!("staging buffer: {name}"),
					allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				},
				meta.total_size,
			)?;
			image.decode_into(upload_buffer.mapped_immediate()?.as_mut_slice());
			upload_buffer
		};

		let image = {
			profiling::scope!("image alloc");
			bindless.image().alloc(&BindlessImageCreateInfo {
				format: select_format(meta),
				extent: Extent::from(meta.extent),
				usage: BindlessImageUsage::SAMPLED | BindlessImageUsage::TRANSFER_DST,
				name,
				..BindlessImageCreateInfo::default()
			})?
		};

		{
			profiling::scope!("image copy cmd");
			Ok(bindless.execute(|cmd| {
				let mut image = image.access::<TransferWrite>(&cmd)?;
				cmd.copy_buffer_to_image(&mut upload_buffer.access::<TransferRead>(&cmd)?, &mut image)?;
				Ok(image.into_shared())
			})?)
		}
	})();
	async { Ok(result?.await) }
}

pub fn select_format(this: RuntimeImageMetadata) -> Format {
	match this.compression {
		RuntimeImageCompression::None => match this.image_type {
			ImageType::RValue => Format::R8_UNORM,
			ImageType::RgValue => Format::R8G8_UNORM,
			ImageType::RgbaLinear => Format::R8G8B8A8_UNORM,
			ImageType::RgbaColor => Format::R8G8B8A8_SRGB,
		},
		RuntimeImageCompression::BCn => match this.image_type {
			ImageType::RValue => Format::BC4_UNORM_BLOCK,
			ImageType::RgValue => Format::BC5_UNORM_BLOCK,
			ImageType::RgbaLinear => Format::BC7_UNORM_BLOCK,
			ImageType::RgbaColor => Format::BC7_SRGB_BLOCK,
		},
	}
}
