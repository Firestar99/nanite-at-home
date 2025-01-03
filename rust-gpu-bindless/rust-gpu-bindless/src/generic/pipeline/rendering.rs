use crate::generic::descriptor::{Bindless, BindlessBufferUsage, Extent, Format};
use crate::generic::pipeline::access_image::MutImageAccess;
use crate::generic::pipeline::access_type::{
	ColorAttachment, DepthStencilAttachment, ImageAccessType, IndexReadable, IndirectCommandReadable,
};
use crate::generic::pipeline::graphics_pipeline::BindlessGraphicsPipeline;
use crate::generic::pipeline::mesh_graphics_pipeline::BindlessMeshGraphicsPipeline;
use crate::generic::pipeline::mut_or_shared::MutOrSharedBuffer;
use crate::generic::pipeline::recording::{HasResourceContext, Recording, RecordingError};
use crate::generic::pipeline::rendering::RenderingError::MismatchedColorAttachmentCount;
use crate::generic::platform::ash::Ash;
use crate::generic::platform::{BindlessPipelinePlatform, RenderingContext};
use crate::spirv_std::indirect_command::{DrawIndexedIndirectCommand, DrawIndirectCommand};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Image2d, TransientAccess};
use smallvec::SmallVec;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use thiserror::Error;

/// A RenderPass defines the formats of the color and depth attachments.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RenderPassFormat {
	pub color_attachments: SmallVec<[Format; 5]>,
	pub depth_attachment: Option<Format>,
}

impl RenderPassFormat {
	pub fn new(color_attachments: &[Format], depth_attachment: Option<Format>) -> Self {
		RenderPassFormat {
			color_attachments: SmallVec::from(color_attachments),
			depth_attachment,
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub enum LoadOp {
	Load,
	Clear(ClearValue),
	DontCare,
}

#[derive(Debug, Copy, Clone)]
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
}

pub struct Rendering<'a: 'b, 'b, P: BindlessPipelinePlatform> {
	platform: P::RenderingContext<'a, 'b>,
}

unsafe impl<'a, 'b, P: BindlessPipelinePlatform> TransientAccess<'a> for Rendering<'a, 'b, P> {}

unsafe impl<'a: 'b, 'b, P: BindlessPipelinePlatform> HasResourceContext<'a, P> for Rendering<'a, 'b, P> {
	#[inline]
	fn bindless(&self) -> &Arc<Bindless<Ash>> {
		self.platform.bindless()
	}

	#[inline]
	fn resource_context(&self) -> &'a P::RecordingResourceContext {
		self.platform.resource_context()
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
			Ok(rendering
				.platform
				.end_rendering()
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}
}

impl<'a: 'b, 'b, P: BindlessPipelinePlatform> Rendering<'a, 'b, P> {
	pub fn draw<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<P, T>,
		count: DrawIndirectCommand,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			Ok(self
				.platform
				.draw(pipeline, count, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}

	pub fn draw_indexed<T: BufferStruct, IT: IndexTypeTrait, AIR: IndexReadable>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<P, T>,
		index_buffer: impl MutOrSharedBuffer<P, [IT], AIR>,
		count: DrawIndexedIndirectCommand,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			index_buffer.has_required_usage(BindlessBufferUsage::INDEX_BUFFER)?;
			Ok(self
				.platform
				.draw_indexed(pipeline, index_buffer, count, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}

	pub fn draw_indirect<T: BufferStruct, AIC: IndirectCommandReadable>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<P, T>,
		indirect: impl MutOrSharedBuffer<P, DrawIndirectCommand, AIC>,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			indirect.has_required_usage(BindlessBufferUsage::INDIRECT_BUFFER)?;
			Ok(self
				.platform
				.draw_indirect(pipeline, indirect, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}

	pub fn draw_indexed_indirect<
		T: BufferStruct,
		IT: IndexTypeTrait,
		AIR: IndexReadable,
		AIC: IndirectCommandReadable,
	>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<P, T>,
		index_buffer: impl MutOrSharedBuffer<P, [IT], AIR>,
		indirect: impl MutOrSharedBuffer<P, DrawIndirectCommand, AIC>,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			index_buffer.has_required_usage(BindlessBufferUsage::INDEX_BUFFER)?;
			indirect.has_required_usage(BindlessBufferUsage::INDIRECT_BUFFER)?;
			Ok(self
				.platform
				.draw_indexed_indirect(pipeline, index_buffer, indirect, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}

	pub fn draw_mesh_tasks<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<P, T>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			Ok(self
				.platform
				.draw_mesh_tasks(pipeline, group_counts, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}

	pub fn draw_mesh_tasks_indirect<T: BufferStruct, AIC: IndirectCommandReadable>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<P, T>,
		indirect: impl MutOrSharedBuffer<P, [u32; 3], AIC>,
		param: T,
	) -> Result<(), RecordingError<P>> {
		unsafe {
			indirect.has_required_usage(BindlessBufferUsage::INDIRECT_BUFFER)?;
			Ok(self
				.platform
				.draw_mesh_tasks_indirect(pipeline, indirect, param)
				.map_err(Into::<RecordingError<P>>::into)?)
		}
	}
}

#[derive(Error)]
#[non_exhaustive]
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

pub enum IndexType {
	U32,
	U16,
}

pub trait IndexTypeTrait: BufferStruct {
	const INDEX_TYPE: IndexType;
}

impl IndexTypeTrait for u32 {
	const INDEX_TYPE: IndexType = IndexType::U32;
}

impl IndexTypeTrait for u16 {
	const INDEX_TYPE: IndexType = IndexType::U16;
}
