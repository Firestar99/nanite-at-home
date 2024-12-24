use crate::descriptor::{Bindless, BindlessBufferUsage, BindlessImageUsage};
use crate::pipeline::access_buffer::MutBufferAccess;
use crate::pipeline::access_error::AccessError;
use crate::pipeline::access_image::MutImageAccess;
use crate::pipeline::access_type::{BufferAccessType, ImageAccessType, TransferReadable, TransferWriteable};
use crate::pipeline::compute_pipeline::BindlessComputePipeline;
use crate::platform::{BindlessPipelinePlatform, RecordingContext};
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{ImageType, TransientAccess};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;

impl<P: BindlessPipelinePlatform> Bindless<P> {
	pub fn execute<R: Send + Sync>(
		self: &Arc<Self>,
		f: impl FnOnce(&mut Recording<'_, P>) -> Result<R, RecordingError<P>>,
	) -> Result<P::ExecutingContext<R>, RecordingError<P>> {
		unsafe { P::record_and_execute(self, f) }
	}
}

pub struct Recording<'a, P: BindlessPipelinePlatform> {
	platform: P::RecordingContext<'a>,
}

unsafe impl<'a, P: BindlessPipelinePlatform> TransientAccess<'a> for Recording<'a, P> {}

impl<'a, P: BindlessPipelinePlatform> Deref for Recording<'a, P> {
	type Target = P::RecordingContext<'a>;

	fn deref(&self) -> &Self::Target {
		&self.platform
	}
}

impl<'a, P: BindlessPipelinePlatform> Recording<'a, P> {
	pub unsafe fn new(platform: P::RecordingContext<'a>) -> Self {
		Self { platform }
	}

	pub unsafe fn inner(&self) -> &P::RecordingContext<'a> {
		&self.platform
	}

	pub unsafe fn inner_mut(&mut self) -> &mut P::RecordingContext<'a> {
		&mut self.platform
	}

	pub unsafe fn into_inner(self) -> P::RecordingContext<'a> {
		self.platform
	}

	pub fn resource_context(&self) -> &'a P::RecordingResourceContext {
		unsafe { self.platform.resource_context() }
	}

	/// Copy data from a buffer to an image. It is assumed that the image data is tightly packed within the buffer.
	/// Partial copies and copying to mips other than mip 0 is not yet possible.
	pub fn copy_buffer_to_image<
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferReadable,
		IT: ImageType,
		IA: ImageAccessType + TransferWriteable,
	>(
		&mut self,
		src_buffer: &mut MutBufferAccess<P, BT, BA>,
		dst_image: &mut MutImageAccess<P, IT, IA>,
	) -> Result<(), RecordingError<P>> {
		self.try_copy_buffer_to_image(src_buffer, dst_image).unwrap()
	}

	/// Copy data from a buffer to an image. It is assumed that the image data is tightly packed within the buffer.
	/// Partial copies and copying to mips other than mip 0 is not yet possible.
	pub fn try_copy_buffer_to_image<
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferReadable,
		IT: ImageType,
		IA: ImageAccessType + TransferWriteable,
	>(
		&mut self,
		src_buffer: &mut MutBufferAccess<P, BT, BA>,
		dst_image: &mut MutImageAccess<P, IT, IA>,
	) -> Result<Result<(), RecordingError<P>>, AccessError> {
		src_buffer.has_required_usage(BindlessBufferUsage::TRANSFER_SRC)?;
		dst_image.has_required_usage(BindlessImageUsage::TRANSFER_DST)?;
		// TODO soundness: missing bounds checks
		unsafe {
			Ok(self
				.platform
				.copy_buffer_to_image(src_buffer, dst_image)
				.map_err(Into::<RecordingError<P>>::into))
		}
	}

	/// Copy data from an image to a buffer. It is assumed that the image data is tightly packed within the buffer.
	/// Partial copies and copying to mips other than mip 0 is not yet possible.
	///
	/// # Safety
	/// This allows any data to be written to the buffer, without checking the buffer's type, potentially transmuting
	/// data.
	pub unsafe fn copy_image_to_buffer<
		IT: ImageType,
		IA: ImageAccessType + TransferReadable,
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src_image: &mut MutImageAccess<P, IT, IA>,
		dst_buffer: &mut MutBufferAccess<P, BT, BA>,
	) -> Result<(), RecordingError<P>> {
		self.try_copy_image_to_buffer(src_image, dst_buffer).unwrap()
	}

	/// Copy data from an image to a buffer. It is assumed that the image data is tightly packed within the buffer.
	/// Partial copies and copying to mips other than mip 0 is not yet possible.
	///
	/// # Safety
	/// This allows any data to be written to the buffer, without checking the buffer's type, potentially transmuting
	/// data.
	pub unsafe fn try_copy_image_to_buffer<
		IT: ImageType,
		IA: ImageAccessType + TransferReadable,
		BT: BufferContent + ?Sized,
		BA: BufferAccessType + TransferWriteable,
	>(
		&mut self,
		src_image: &mut MutImageAccess<P, IT, IA>,
		dst_buffer: &mut MutBufferAccess<P, BT, BA>,
	) -> Result<Result<(), RecordingError<P>>, AccessError> {
		src_image.has_required_usage(BindlessImageUsage::TRANSFER_SRC)?;
		dst_buffer.has_required_usage(BindlessBufferUsage::TRANSFER_DST)?;
		// TODO soundness: missing bounds checks
		unsafe {
			Ok(self
				.platform
				.copy_image_to_buffer(src_image, dst_buffer)
				.map_err(Into::<RecordingError<P>>::into))
		}
	}

	/// Dispatch a bindless compute shader
	pub fn dispatch<T: BufferStruct>(
		&mut self,
		pipeline: &Arc<BindlessComputePipeline<P, T>>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			self.platform
				.dispatch(pipeline, group_counts, param)
				.map_err(Into::<RecordingError<P>>::into)?;
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum RecordingError<P: BindlessPipelinePlatform> {
	#[error("Platform Error: {0}")]
	Platform(#[source] P::RecordingError),
	#[error("Copy Error: {0}")]
	CopyError(#[from] CopyError),
}

impl<P: BindlessPipelinePlatform> Debug for RecordingError<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}

#[derive(Error)]
pub enum CopyError {}

impl Debug for CopyError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}
