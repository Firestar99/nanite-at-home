use crate::image::upload::upload_image2d_disk;
use glam::Vec4;
use pollster::block_on;
use rkyv::api::high::HighDeserializer;
use rkyv::rancor::Panic;
use rkyv::Deserialize;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, RCDesc, RC,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Buffer, Desc, Image, Image2d};
use space_asset_disk::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, ImageType, Size};
use std::future::Future;

pub fn deserialize_infallible<A, T>(a: &A) -> T
where
	A: Deserialize<T, HighDeserializer<Panic>>,
{
	rkyv::deserialize(a).unwrap()
}

pub struct Uploader {
	pub bindless: Bindless,
	default_white_texture: Option<RCDesc<Image<Image2d>>>,
	default_normal_texture: Option<RCDesc<Image<Image2d>>>,
}

impl Uploader {
	pub fn new(bindless: Bindless) -> Self {
		let mut uploader = Self {
			bindless,
			default_white_texture: None,
			default_normal_texture: None,
		};

		let default_texture = |uploader: &Uploader, name: &str, color: Vec4| {
			let color = color.to_array().map(|f| (f * 255.) as u8);
			let bytes = Vec::from(color).into();
			let disk = Image2DDisk::<{ ImageType::RgbaLinear as u32 }> {
				metadata: Image2DMetadata {
					size: Size::new(1, 1),
					disk_compression: DiskImageCompression::None,
				},
				bytes,
			};
			block_on(upload_image2d_disk(&disk, name, &uploader)).unwrap()
		};

		uploader.default_white_texture = Some(default_texture(&uploader, "default_white_texture", Vec4::splat(1.)));
		uploader.default_normal_texture = Some(default_texture(&uploader, "default_normal_texture", Vec4::splat(0.5)));
		uploader
	}

	// TODO eventually these should use a staging buffer to upload to non-host accessible memory, if required
	pub fn upload_buffer_data<T: BufferStruct + 'static>(
		&self,
		name: &str,
		data: T,
	) -> impl Future<Output = anyhow::Result<RCDesc<Buffer<T>>>> + '_ {
		let buffer = self.bindless.buffer().alloc_shared_from_data(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER | BindlessBufferUsage::MAP_WRITE,
				name,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			data,
		);
		async { Ok(buffer?) }
	}

	pub fn upload_buffer_iter<T: BufferStruct + 'static, I>(
		&self,
		name: &str,
		iter: I,
	) -> impl Future<Output = anyhow::Result<RCDesc<Buffer<[T]>>>> + '_
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let buffer = self.bindless.buffer().alloc_shared_from_iter(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER | BindlessBufferUsage::MAP_WRITE,
				name,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			iter,
		);
		async { Ok(buffer?) }
	}

	pub fn default_white_texture(&self) -> RCDesc<Image<Image2d>> {
		self.default_white_texture.as_ref().unwrap().clone()
	}

	pub fn default_normal_texture(&self) -> RCDesc<Image<Image2d>> {
		self.default_normal_texture.as_ref().unwrap().clone()
	}

	pub async fn await_or_default_texture(
		&self,
		tex: Option<impl Future<Output = anyhow::Result<Desc<RC, Image<Image2d>>>>>,
		default: impl FnOnce(&Self) -> Desc<RC, Image<Image2d>>,
	) -> anyhow::Result<Desc<RC, Image<Image2d>>> {
		if let Some(tex) = tex {
			Ok(tex.await?)
		} else {
			Ok(default(self))
		}
	}
}
