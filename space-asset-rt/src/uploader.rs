use crate::image::upload::upload_image2d_disk;
use async_std::task::block_on;
use rkyv::Deserialize;
use space_asset_disk::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, ImageType, Size};
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::sync::Arc;
use vulkano::buffer::{AllocateBufferError, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::buffer::{Buffer as VBuffer, Subbuffer};
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferExecError, CommandBufferLevel, CommandBufferUsage, CopyBufferInfo,
	RecordingCommandBuffer,
};
use vulkano::device::Queue;
use vulkano::image::AllocateImageError;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter};
use vulkano::sync::GpuFuture;
use vulkano::{Validated, ValidationError, VulkanError};
use vulkano_bindless::descriptor::buffer_metadata_cpu::{BackingRefsError, StrongMetadataCpu};
use vulkano_bindless::descriptor::{Bindless, RCDesc, StrongBackingRefs, RC};
use vulkano_bindless::spirv_std::image::Image2d;
use vulkano_bindless_shaders::buffer_content::{BufferContent, BufferStruct, Metadata};
use vulkano_bindless_shaders::descriptor::{Buffer, Desc};
use zune_image::errors::ImageErrors;

pub fn deserialize_infallible<A, T>(a: &A) -> T
where
	A: Deserialize<T, rkyv::Infallible>,
{
	let t: T = a.deserialize(&mut rkyv::Infallible).unwrap();
	t
}

pub struct Uploader {
	pub bindless: Arc<Bindless>,
	pub memory_allocator: Arc<dyn MemoryAllocator>,
	pub cmd_allocator: Arc<dyn CommandBufferAllocator>,
	pub transfer_queue: Arc<Queue>,

	white_texture: Option<Desc<RC, Image2d>>,
}

impl Uploader {
	pub fn new(
		bindless: Arc<Bindless>,
		memory_allocator: Arc<dyn MemoryAllocator>,
		cmd_allocator: Arc<dyn CommandBufferAllocator>,
		transfer_queue: Arc<Queue>,
	) -> Self {
		let mut uploader = Self {
			bindless,
			memory_allocator,
			cmd_allocator,
			transfer_queue,
			white_texture: None,
		};
		let white_texture = {
			let disk = Image2DDisk::<{ ImageType::RGBA_LINEAR as u32 }> {
				metadata: Image2DMetadata {
					size: Size::new(1, 1),
					disk_compression: DiskImageCompression::None,
				},
				bytes: Vec::from([255, 255, 255, 255]).into(),
			};
			block_on(upload_image2d_disk(&disk, &uploader)).unwrap()
		};
		uploader.white_texture = Some(white_texture);
		uploader
	}

	pub fn upload_buffer_data<T: BufferStruct + 'static>(
		&self,
		data: T,
	) -> impl Future<Output = Result<RCDesc<Buffer<T>>, Validated<UploadError>>> + '_ {
		let result: Result<_, Validated<UploadError>> = (|| unsafe {
			profiling::scope!("data upload to host buffer");
			let mut meta = StrongMetadataCpu::new(&self.bindless, Metadata);
			let upload_buffer = VBuffer::from_data(
				self.memory_allocator.clone(),
				BufferCreateInfo {
					usage: BufferUsage::TRANSFER_SRC,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				T::write_cpu(data, &mut meta),
			)
			.map_err(UploadError::from_validated)?;
			let backing_refs = meta.into_backing_refs().map_err(UploadError::from_validated)?;
			Ok(self.upload_buffer(upload_buffer, backing_refs))
		})();
		async { result?.await }
	}

	pub fn upload_buffer_iter<T: BufferStruct + 'static, I>(
		&self,
		iter: I,
	) -> impl Future<Output = Result<RCDesc<Buffer<[T]>>, Validated<UploadError>>> + '_
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let result: Result<_, Validated<UploadError>> = (|| unsafe {
			profiling::scope!("iter upload to host buffer");
			let mut meta = StrongMetadataCpu::new(&self.bindless, Metadata);
			let upload_buffer = VBuffer::from_iter(
				self.memory_allocator.clone(),
				BufferCreateInfo {
					usage: BufferUsage::TRANSFER_SRC,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				iter.into_iter().map(|i| T::write_cpu(i, &mut meta)),
			)
			.map_err(UploadError::from_validated)?;
			let backing_refs = meta.into_backing_refs().map_err(UploadError::from_validated)?;
			Ok(self.upload_buffer(upload_buffer, backing_refs))
		})();
		async { result?.await }
	}

	fn upload_buffer<T: BufferContent + ?Sized + 'static>(
		&self,
		upload_buffer: Subbuffer<T::Transfer>,
		backing_refs: StrongBackingRefs,
	) -> impl Future<Output = Result<RCDesc<Buffer<T>>, Validated<UploadError>>> + '_
	where
		T::Transfer: BufferContents,
	{
		let result: Result<_, Validated<UploadError>> = (|| {
			let perm_buffer;
			let cmd = {
				profiling::scope!("buffer copy cmd record");
				perm_buffer = VBuffer::new_slice::<u8>(
					self.memory_allocator.clone(),
					BufferCreateInfo {
						usage: BufferUsage::TRANSFER_DST | BufferUsage::STORAGE_BUFFER,
						..BufferCreateInfo::default()
					},
					AllocationCreateInfo {
						memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
						..AllocationCreateInfo::default()
					},
					upload_buffer.size(),
				)
				.map_err(UploadError::from_validated)?;

				let mut cmd = RecordingCommandBuffer::new(
					self.cmd_allocator.clone(),
					self.transfer_queue.queue_family_index(),
					CommandBufferLevel::Primary,
					CommandBufferBeginInfo {
						usage: CommandBufferUsage::OneTimeSubmit,
						..CommandBufferBeginInfo::default()
					},
				)
				.map_err(UploadError::from_validated)?;
				cmd.copy_buffer(CopyBufferInfo::buffers(upload_buffer, perm_buffer.clone()))
					.map_err(UploadError::from_validated)?;
				cmd.end().map_err(UploadError::from_validated)?
			};
			profiling::scope!("buffer copy cmd submit");
			let fence = cmd
				.execute(self.transfer_queue.clone())
				.map_err(UploadError::from_validated)?
				.then_signal_fence_and_flush()
				.map_err(UploadError::from_validated)?;
			Ok((perm_buffer, fence))
		})();

		async {
			let (perm_buffer, fence) = result?;
			fence.await.map_err(UploadError::from_validated)?;
			Ok(self
				.bindless
				.buffer()
				.alloc_slot(perm_buffer.reinterpret(), backing_refs))
		}
	}

	pub fn white_texture(&self) -> Desc<RC, Image2d> {
		self.white_texture.as_ref().unwrap().clone()
	}

	pub async fn await_or_white_texture(
		&self,
		tex: Option<impl Future<Output = Result<Desc<RC, Image2d>, Validated<UploadError>>>>,
	) -> Result<Desc<RC, Image2d>, Validated<UploadError>> {
		if let Some(tex) = tex {
			tex.await
		} else {
			Ok(self.white_texture())
		}
	}
}

#[derive(Debug)]
pub enum UploadError {
	AllocateBufferError(AllocateBufferError),
	AllocateImageError(AllocateImageError),
	VulkanError(VulkanError),
	CommandBufferExecError(CommandBufferExecError),
	BackingRefsError(BackingRefsError),
	ImageErrors(ImageErrors),
}

pub trait ValidatedFrom<T>: Sized {
	fn from_validated(value: T) -> Validated<Self>;
}

impl ValidatedFrom<BackingRefsError> for UploadError {
	fn from_validated(value: BackingRefsError) -> Validated<Self> {
		Validated::Error(Self::BackingRefsError(value))
	}
}

impl ValidatedFrom<Validated<VulkanError>> for UploadError {
	fn from_validated(value: Validated<VulkanError>) -> Validated<Self> {
		match value {
			Validated::Error(e) => Validated::Error(Self::VulkanError(e)),
			Validated::ValidationError(v) => Validated::ValidationError(v),
		}
	}
}

impl ValidatedFrom<VulkanError> for UploadError {
	fn from_validated(value: VulkanError) -> Validated<Self> {
		Validated::Error(Self::VulkanError(value))
	}
}

impl ValidatedFrom<Validated<AllocateBufferError>> for UploadError {
	fn from_validated(value: Validated<AllocateBufferError>) -> Validated<Self> {
		match value {
			Validated::Error(e) => Validated::Error(Self::AllocateBufferError(e)),
			Validated::ValidationError(v) => Validated::ValidationError(v),
		}
	}
}

impl ValidatedFrom<Validated<AllocateImageError>> for UploadError {
	fn from_validated(value: Validated<AllocateImageError>) -> Validated<Self> {
		match value {
			Validated::Error(e) => Validated::Error(Self::AllocateImageError(e)),
			Validated::ValidationError(v) => Validated::ValidationError(v),
		}
	}
}

impl ValidatedFrom<CommandBufferExecError> for UploadError {
	fn from_validated(value: CommandBufferExecError) -> Validated<Self> {
		Validated::Error(Self::CommandBufferExecError(value))
	}
}

impl ValidatedFrom<Box<ValidationError>> for UploadError {
	fn from_validated(value: Box<ValidationError>) -> Validated<Self> {
		Validated::ValidationError(value)
	}
}

impl ValidatedFrom<ImageErrors> for UploadError {
	fn from_validated(value: ImageErrors) -> Validated<Self> {
		Validated::Error(Self::ImageErrors(value))
	}
}

impl Display for UploadError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			UploadError::AllocateBufferError(e) => Display::fmt(e, f),
			UploadError::AllocateImageError(e) => Display::fmt(e, f),
			UploadError::VulkanError(e) => Display::fmt(e, f),
			UploadError::CommandBufferExecError(e) => Display::fmt(e, f),
			UploadError::BackingRefsError(e) => Display::fmt(e, f),
			UploadError::ImageErrors(e) => Display::fmt(e, f),
		}
	}
}
