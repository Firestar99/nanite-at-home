use crate::descriptor::{Extent, Format};
use crate::pipeline::access_image::MutImageAccess;
use crate::pipeline::access_type::{ColorAttachment, DepthStencilAttachment, ImageAccessType};
use crate::pipeline::graphics_pipeline::BindlessGraphicsPipeline;
use crate::pipeline::recording::{HasResourceContext, Recording, RecordingError};
use crate::pipeline::rendering::RenderingError::MismatchedColorAttachmentCount;
use crate::platform::{BindlessPipelinePlatform, RenderingContext};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Image2d, TransientAccess};
use smallvec::SmallVec;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;

/// A RenderPass defines the formats of the color and depth attachments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RenderPassFormat {
	color_attachments: SmallVec<[Format; 5]>,
	depth_attachment: Option<Format>,
}

impl RenderPassFormat {
	pub fn new(color_attachments: &[Format], depth_attachment: Option<Format>) -> Self {
		RenderPassFormat {
			color_attachments: SmallVec::from(color_attachments),
			depth_attachment,
		}
	}

	pub fn color_attachments(&self) -> &[Format] {
		&self.color_attachments
	}

	pub fn depth_attachment(&self) -> Option<Format> {
		self.depth_attachment
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum LoadOp {
	Load,
	Clear,
	DontCare,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum StoreOp {
	Store,
	DontCare,
}

#[derive(Debug, Copy, Clone)]
pub enum ClearValue {
	ColorF([f32; 4]),
	ColorU([u32; 4]),
	ColorI([i32; 4]),
	DepthStencil { depth: f32, stencil: u32 },
}

pub struct RenderingAttachment<'a, 'b, P: BindlessPipelinePlatform, A: ImageAccessType> {
	pub image: &'b mut MutImageAccess<'a, P, Image2d, A>,
	pub load_op: LoadOp,
	pub store_op: StoreOp,
	pub clear_value: ClearValue,
}

pub struct Rendering<'a: 'b, 'b, P: BindlessPipelinePlatform> {
	platform: P::RenderingContext<'a, 'b>,
}

unsafe impl<'a, 'b, P: BindlessPipelinePlatform> TransientAccess<'a> for Rendering<'a, 'b, P> {}

unsafe impl<'a: 'b, 'b, P: BindlessPipelinePlatform> HasResourceContext<'a, P> for Rendering<'a, 'b, P> {
	unsafe fn resource_context(&self) -> &'a P::RecordingResourceContext {
		unsafe { self.platform.resource_context() }
	}
}

impl<'a, P: BindlessPipelinePlatform> Recording<'a, P> {
	pub fn begin_rendering(
		&mut self,
		format: RenderPassFormat,
		color_attachments: &[RenderingAttachment<'a, '_, P, ColorAttachment>],
		depth_attachment: Option<RenderingAttachment<'a, '_, P, DepthStencilAttachment>>,
		f: impl FnOnce(&mut Rendering<'a, '_, P>) -> Result<(), RecordingError<P>>,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			let extent = if let Some(depth_format) = format.depth_attachment {
				if let Some(depth_attachment) = &depth_attachment {
					let slot = depth_attachment.image.inner_slot();
					if slot.format != depth_format {
						return Err(RenderingError::MismatchedDepthAttachmentFormat {
							name: slot.debug_name.to_string(),
							format: slot.format,
							expected: depth_format,
						}
						.into());
					}
					slot.extent
				} else {
					return Err(RenderingError::DepthAttachmentMissing.into());
				}
			} else {
				if let Some(depth_attachment) = &depth_attachment {
					return Err(RenderingError::DepthAttachmentNotExpected {
						name: depth_attachment.image.inner_slot().debug_name.to_string(),
					}
					.into());
				} else {
					if let Some(color_attachment1) = color_attachments.first() {
						color_attachment1.image.inner_slot().extent
					} else {
						return Err(RenderingError::NoAttachments.into());
					}
				}
			};

			if color_attachments.len() != format.color_attachments.len() {
				return Err(MismatchedColorAttachmentCount {
					count: color_attachments.len(),
					expected: format.color_attachments.len(),
				}
				.into());
			}

			for (index, x) in color_attachments.iter().enumerate() {
				let slot = x.image.inner_slot();
				let exp_format = format.color_attachments[index];
				if slot.format != exp_format {
					return Err(RenderingError::MismatchedColorAttachmentFormat {
						index,
						name: slot.debug_name.to_string(),
						format: slot.format,
						expected: exp_format,
					}
					.into());
				}
				if slot.extent != extent {
					return Err(RenderingError::MismatchedColorAttachmentSize {
						index,
						name: slot.debug_name.to_string(),
						size: slot.extent,
						expected_size: extent,
					}
					.into());
				}
			}

			let mut rendering = Rendering {
				platform: <P::RenderingContext<'a, '_> as RenderingContext<P>>::begin_rendering(
					self.inner_mut(),
					format,
					[extent.width, extent.height],
					color_attachments,
					depth_attachment,
				)
				.map_err(Into::<RecordingError<P>>::into)?,
			};
			f(&mut rendering)?;
			rendering
				.platform
				.end_rendering()
				.map_err(Into::<RecordingError<P>>::into)?;
			Ok(())
		}
	}
}

impl<'a: 'b, 'b, P: BindlessPipelinePlatform> Rendering<'a, 'b, P> {
	pub fn draw<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<P, T>,
		vertex_count: u32,
		instance_count: u32,
		first_vertex: u32,
		first_instance: u32,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			self.platform
				.draw(
					pipeline,
					vertex_count,
					instance_count,
					first_vertex,
					first_instance,
					param,
				)
				.map_err(Into::<RecordingError<P>>::into)?;
			Ok(())
		}
	}
}

#[derive(Error)]
pub enum RenderingError {
	#[error("At least one attachment expected to perform rendering")]
	NoAttachments,
	#[error("At least one attachment expected to perform rendering")]
	MismatchedColorAttachmentCount { count: usize, expected: usize },
	#[error("Depth attachment missing, but was declared in RenderPassFormat")]
	DepthAttachmentMissing,
	#[error("Depth attachment \"{name}\" present, but no Depth Attachment was declared in RenderPassFormat")]
	DepthAttachmentNotExpected { name: String },
	#[error("Depth attachment \"{name}\" has format {format:?} but format {expected:?} was expected")]
	MismatchedDepthAttachmentFormat {
		name: String,
		format: Format,
		expected: Format,
	},
	#[error("Color attachment {index} \"{name}\" has format {format:?} but format {expected:?} was expected")]
	MismatchedColorAttachmentFormat {
		index: usize,
		name: String,
		format: Format,
		expected: Format,
	},
	#[error("Color attachment {index} \"{name}\" has size {size:?} but was expected to have a common size of {expected_size:?}"
	)]
	MismatchedColorAttachmentSize {
		index: usize,
		name: String,
		size: Extent,
		expected_size: Extent,
	},
}

impl Debug for RenderingError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}
