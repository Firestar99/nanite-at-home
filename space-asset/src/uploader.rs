#![cfg(feature = "runtime")]

use crate::image::{ArchivedImage2DDisk, Image2DMetadata, ImageValidationError};
use rkyv::Deserialize;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use vulkano::buffer::{AllocateBufferError, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::buffer::{Buffer as VBuffer, Subbuffer};
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferExecError, CommandBufferLevel, CommandBufferUsage, CopyBufferInfo,
	CopyBufferToImageInfo, RecordingCommandBuffer,
};
use vulkano::device::Queue;
use vulkano::image::view::ImageView;
use vulkano::image::{AllocateImageError, Image as VImage, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter};
use vulkano::sync::GpuFuture;
use vulkano::{DeviceSize, Validated, ValidationError, VulkanError};
use vulkano_bindless::descriptor::buffer_metadata_cpu::{BackingRefsError, StrongMetadataCpu};
use vulkano_bindless::descriptor::{Bindless, RCDesc, StrongBackingRefs};
use vulkano_bindless::spirv_std::image::Image2d;
use vulkano_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use vulkano_bindless_shaders::descriptor::metadata::Metadata;
use vulkano_bindless_shaders::descriptor::Buffer;

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
}

impl Uploader {
	pub async fn upload_buffer_data<T: BufferStruct>(
		&self,
		data: T,
	) -> Result<RCDesc<Buffer<T>>, Validated<UploadError>> {
		let upload_buffer;
		let backing_refs;
		unsafe {
			profiling::scope!("data upload to host buffer");
			let mut meta = StrongMetadataCpu::new(&self.bindless, Metadata);
			upload_buffer = VBuffer::from_data(
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
			backing_refs = meta.into_backing_refs().map_err(UploadError::from_validated)?;
		}
		self.upload_buffer(upload_buffer, backing_refs).await
	}

	pub async fn upload_buffer_iter<T: BufferStruct, I>(
		&self,
		iter: I,
	) -> Result<RCDesc<Buffer<[T]>>, Validated<UploadError>>
	where
		I: IntoIterator<Item = T>,
		I::IntoIter: ExactSizeIterator,
	{
		let upload_buffer;
		let backing_refs;
		unsafe {
			profiling::scope!("iter upload to host buffer");
			let mut meta = StrongMetadataCpu::new(&self.bindless, Metadata);
			upload_buffer = VBuffer::from_iter(
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
			backing_refs = meta.into_backing_refs().map_err(UploadError::from_validated)?;
		}
		self.upload_buffer(upload_buffer, backing_refs).await
	}

	async fn upload_buffer<T: BufferContent + ?Sized>(
		&self,
		upload_buffer: Subbuffer<T::Transfer>,
		backing_refs: StrongBackingRefs,
	) -> Result<RCDesc<Buffer<T>>, Validated<UploadError>>
	where
		T::Transfer: BufferContents,
	{
		let perm_buffer;
		{
			profiling::scope!("buffer copy cmd");
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

			let cmd = {
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
			cmd.execute(self.transfer_queue.clone())
				.map_err(UploadError::from_validated)?
				.then_signal_fence_and_flush()
				.map_err(UploadError::from_validated)?
		}
		.await
		.map_err(UploadError::from_validated)?;

		Ok(self
			.bindless
			.buffer()
			.alloc_slot(perm_buffer.reinterpret(), backing_refs))
	}

	pub async fn upload_image2d(&self, image: &ArchivedImage2DDisk) -> Result<RCDesc<Image2d>, Validated<UploadError>> {
		let metadata = deserialize_infallible::<_, Image2DMetadata>(&image.metadata);
		metadata
			.validate(image.bytes.len())
			.map_err(UploadError::from_validated)?;

		let upload_buffer = {
			profiling::scope!("image upload to host buffer");
			let upload_buffer = VBuffer::new_slice::<u8>(
				self.memory_allocator.clone(),
				BufferCreateInfo {
					usage: BufferUsage::TRANSFER_SRC,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				image.bytes.len() as DeviceSize,
			)
			.map_err(UploadError::from_validated)?;
			upload_buffer.write().unwrap().copy_from_slice(&image.bytes);
			upload_buffer
		};

		let perm_image;
		{
			profiling::scope!("image copy cmd");
			perm_image = VImage::new(
				self.memory_allocator.clone(),
				ImageCreateInfo {
					image_type: ImageType::Dim2d,
					format: metadata.encoding.vulkano_format(),
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
					self.cmd_allocator.clone(),
					self.transfer_queue.queue_family_index(),
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
			cmd.execute(self.transfer_queue.clone())
				.map_err(UploadError::from_validated)?
				.then_signal_fence_and_flush()
				.map_err(UploadError::from_validated)?
		}
		.await
		.map_err(UploadError::from_validated)?;

		Ok(self
			.bindless
			.image()
			.alloc_slot_2d(ImageView::new_default(perm_image).map_err(UploadError::from_validated)?))
	}
}

#[derive(Debug)]
pub enum UploadError {
	AllocateBufferError(AllocateBufferError),
	AllocateImageError(AllocateImageError),
	VulkanError(VulkanError),
	CommandBufferExecError(CommandBufferExecError),
	BackingRefsError(BackingRefsError),
	ImageValidationError(ImageValidationError),
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

impl ValidatedFrom<ImageValidationError> for UploadError {
	fn from_validated(value: ImageValidationError) -> Validated<Self> {
		Validated::Error(Self::ImageValidationError(value))
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
			UploadError::ImageValidationError(e) => Display::fmt(e, f),
		}
	}
}
