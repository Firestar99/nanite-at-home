use rkyv::Deserialize;
use rkyv::api::high::HighDeserializer;
use rkyv::rancor::Panic;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, RC, RCDesc,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Buffer, Desc, Image, Image2d};
use std::future::Future;

pub fn deserialize_infallible<A, T>(a: &A) -> T
where
	A: Deserialize<T, HighDeserializer<Panic>>,
{
	rkyv::deserialize(a).unwrap()
}

pub struct Uploader {
	pub bindless: Bindless,
}

impl Uploader {
	pub fn new(bindless: Bindless) -> Self {
		Self { bindless }
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
