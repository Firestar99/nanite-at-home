use crate::descriptor::Bindless;
use crate::pipeline::{
	BindlessGraphicsPipeline, BindlessMeshGraphicsPipeline, ColorAttachment, DepthStencilAttachment,
	DrawIndexedIndirectCommand, DrawIndirectCommand, HasResourceContext, IndexReadable, IndexTypeTrait,
	IndirectCommandReadable, MutOrSharedBuffer, RecordingError, RenderPassFormat, RenderingAttachment,
};
use crate::platform::RenderingContext;
use crate::platform::ash::bindless_pipeline::AshPipeline;
use crate::platform::ash::{Ash, AshRecordingContext, AshRecordingError, AshRecordingResourceContext};
use ash::vk::{Extent2D, ImageLayout, Offset2D, PipelineBindPoint, Rect2D, RenderingAttachmentInfo, RenderingInfo};
use glam::UVec2;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::TransientAccess;
use rust_gpu_bindless_shaders::utils::rect::IRect2;
use rust_gpu_bindless_shaders::utils::viewport::Viewport;
use smallvec::SmallVec;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};

pub struct AshRenderingContext<'a, 'b> {
	recording: &'b mut AshRecordingContext<'a>,
	graphics_bind_descriptors: bool,
	viewport: Viewport,
	scissor: IRect2,
	set_viewport: bool,
	set_scissor: bool,
}

impl<'a> Deref for AshRenderingContext<'a, '_> {
	type Target = AshRecordingContext<'a>;

	fn deref(&self) -> &Self::Target {
		self.recording
	}
}

impl DerefMut for AshRenderingContext<'_, '_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.recording
	}
}

impl<'a, 'b> AshRenderingContext<'a, 'b> {
	pub unsafe fn new(recording: &'b mut AshRecordingContext<'a>) -> Self {
		Self {
			recording,
			graphics_bind_descriptors: true,
			viewport: Viewport::default(),
			scissor: IRect2::default(),
			set_viewport: true,
			set_scissor: true,
		}
	}

	/// Invalidates internal state that keeps track of the command buffer's state. Currently, it forces the global
	/// descriptor set, current viewport and scissor rect to be rebound.
	pub fn ash_invalidate_graphics(&mut self) {
		self.ash_invalidate_graphics_descriptor_set();
		self.ash_invalidate_graphics_viewport();
		self.ash_invalidate_graphics_scissor();
	}

	/// Invalidates internal state that keeps track of the command buffer's state for the global descriptor set.
	#[inline]
	pub fn ash_invalidate_graphics_descriptor_set(&mut self) {
		self.graphics_bind_descriptors = true;
	}

	/// Invalidates internal state that keeps track of the command buffer's state for the viewport.
	#[inline]
	pub fn ash_invalidate_graphics_viewport(&mut self) {
		self.set_viewport = true;
	}

	/// Invalidates internal state that keeps track of the command buffer's state for the scissor rect.
	#[inline]
	pub fn ash_invalidate_graphics_scissor(&mut self) {
		self.set_scissor = true;
	}

	/// Flushes the following graphics state changes to the command buffer:
	/// * verify no barrier flushes are queued, illegal inside render passes
	/// * flush viewport
	/// * flush scissor
	pub unsafe fn ash_flush_graphics(&mut self) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_must_not_flush_barriers()?;
			self.ash_flush_viewport();
			self.ash_flush_scissor();
			Ok(())
		}
	}

	pub unsafe fn ash_flush_viewport(&mut self) {
		if self.set_viewport {
			self.set_viewport = false;

			unsafe {
				let device = &self.recording.bindless.platform.device;
				let viewport = self.viewport;
				device.cmd_set_viewport(
					self.recording.cmd,
					0,
					&[ash::vk::Viewport {
						x: viewport.x,
						y: viewport.y,
						width: viewport.width,
						height: viewport.height,
						min_depth: viewport.min_depth,
						max_depth: viewport.max_depth,
					}],
				);
			}
		}
	}

	pub unsafe fn ash_flush_scissor(&mut self) {
		if self.set_scissor {
			self.set_scissor = false;

			unsafe {
				let device = &self.recording.bindless.platform.device;
				let scissor = self.scissor;
				device.cmd_set_scissor(
					self.recording.cmd,
					0,
					&[Rect2D {
						offset: Offset2D {
							x: scissor.origin.x,
							y: scissor.origin.y,
						},
						extent: Extent2D {
							width: scissor.extent.x,
							height: scissor.extent.y,
						},
					}],
				);
			}
		}
	}

	#[inline]
	pub unsafe fn ash_bind_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe { self.ash_bind_any_graphics(&pipeline.inner().0, param) }
	}

	#[inline]
	pub unsafe fn ash_bind_mesh_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<Ash, T>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe { self.ash_bind_any_graphics(&pipeline.inner().0, param) }
	}

	pub unsafe fn ash_bind_any_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &AshPipeline,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_flush_graphics()?;
			let device = &self.recording.bindless.platform.device;
			device.cmd_bind_pipeline(self.cmd, PipelineBindPoint::GRAPHICS, pipeline.pipeline);
			if self.graphics_bind_descriptors {
				self.graphics_bind_descriptors = false;
				let desc = self.bindless.global_descriptor_set();
				device.cmd_bind_descriptor_sets(
					self.cmd,
					PipelineBindPoint::GRAPHICS,
					desc.pipeline_layout,
					0,
					&[desc.set],
					&[],
				);
			}
			self.ash_push_param(param);
			Ok(())
		}
	}
}

unsafe impl<'a> TransientAccess<'a> for AshRenderingContext<'a, '_> {}

unsafe impl<'a> HasResourceContext<'a, Ash> for AshRenderingContext<'a, '_> {
	#[inline]
	fn bindless(&self) -> &Bindless<Ash> {
		self.recording.bindless()
	}

	#[inline]
	fn resource_context(&self) -> &'a AshRecordingResourceContext {
		self.recording.resource_context()
	}
}

unsafe impl<'a, 'b> RenderingContext<'a, 'b, Ash> for AshRenderingContext<'a, 'b> {
	unsafe fn begin_rendering(
		recording: &'b mut AshRecordingContext<'a>,
		_format: RenderPassFormat,
		render_area: UVec2,
		color_attachments: &[RenderingAttachment<Ash, ColorAttachment>],
		depth_attachment: Option<RenderingAttachment<Ash, DepthStencilAttachment>>,
	) -> Result<Self, AshRecordingError> {
		unsafe {
			recording.ash_flush();
			let device = &recording.bindless.platform.device;
			device.cmd_begin_rendering(
				recording.cmd,
				&RenderingInfo::default()
					.render_area(Rect2D {
						offset: Offset2D { x: 0, y: 0 },
						extent: Extent2D {
							width: render_area.x,
							height: render_area.y,
						},
					})
					.layer_count(1)
					.color_attachments(
						&color_attachments
							.iter()
							.map(|c| c.to_ash(ImageLayout::COLOR_ATTACHMENT_OPTIMAL))
							.collect::<SmallVec<[_; 5]>>(),
					)
					.depth_attachment(&if let Some(c) = &depth_attachment {
						c.to_ash(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
					} else {
						RenderingAttachmentInfo::default()
					}),
			);
			Ok(Self::new(recording))
		}
	}

	unsafe fn end_rendering(&mut self) -> Result<(), AshRecordingError> {
		unsafe {
			let device = &self.bindless.platform.device;
			device.cmd_end_rendering(self.cmd);
			Ok(())
		}
	}

	unsafe fn set_viewport(&mut self, viewport: Viewport) {
		self.viewport = viewport;
		self.set_viewport = true;
	}

	unsafe fn set_scissor(&mut self, scissor: IRect2) {
		self.scissor = scissor;
		self.set_scissor = true;
	}

	unsafe fn draw<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		count: DrawIndirectCommand,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_graphics(pipeline, param)?;
			let device = &self.bindless.platform.device;
			device.cmd_draw(
				self.cmd,
				count.vertex_count,
				count.instance_count,
				count.first_vertex,
				count.first_instance,
			);
			Ok(())
		}
	}

	unsafe fn draw_indexed<T: BufferStruct, IT: IndexTypeTrait, AIR: IndexReadable>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		index_buffer: impl MutOrSharedBuffer<Ash, [IT], AIR>,
		count: DrawIndexedIndirectCommand,
		param: T,
	) -> Result<(), RecordingError<Ash>> {
		unsafe {
			self.ash_bind_graphics(pipeline, param)?;
			let device = &self.bindless.platform.device;
			device.cmd_bind_index_buffer(
				self.cmd,
				index_buffer.inner_slot().buffer,
				0,
				IT::INDEX_TYPE.to_ash_index_type(),
			);
			device.cmd_draw_indexed(
				self.cmd,
				count.index_count,
				count.instance_count,
				count.first_index,
				count.vertex_offset,
				count.first_instance,
			);
			Ok(())
		}
	}

	unsafe fn draw_indirect<T: BufferStruct, AIC: IndirectCommandReadable>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		indirect: impl MutOrSharedBuffer<Ash, DrawIndirectCommand, AIC>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_graphics(pipeline, param)?;
			let device = &self.bindless.platform.device;
			let indirect = indirect.inner_slot();
			device.cmd_draw_indirect(self.cmd, indirect.buffer, 0, 1, size_of::<DrawIndirectCommand>() as u32);
			Ok(())
		}
	}

	unsafe fn draw_indexed_indirect<
		T: BufferStruct,
		IT: IndexTypeTrait,
		AIR: IndexReadable,
		AIC: IndirectCommandReadable,
	>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		index_buffer: impl MutOrSharedBuffer<Ash, [IT], AIR>,
		indirect: impl MutOrSharedBuffer<Ash, DrawIndexedIndirectCommand, AIC>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_graphics(pipeline, param)?;
			let device = &self.bindless.platform.device;
			let indirect = indirect.inner_slot();
			device.cmd_bind_index_buffer(
				self.cmd,
				index_buffer.inner_slot().buffer,
				0,
				IT::INDEX_TYPE.to_ash_index_type(),
			);
			device.cmd_draw_indexed_indirect(
				self.cmd,
				indirect.buffer,
				0,
				1,
				size_of::<DrawIndexedIndirectCommand>() as u32,
			);
			Ok(())
		}
	}

	unsafe fn draw_mesh_tasks<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<Ash, T>,
		group_counts: [u32; 3],
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_mesh_graphics(pipeline, param)?;
			let device = &self.bindless.platform.extensions.mesh_shader();
			device.cmd_draw_mesh_tasks(self.cmd, group_counts[0], group_counts[1], group_counts[2]);
			Ok(())
		}
	}

	unsafe fn draw_mesh_tasks_indirect<T: BufferStruct, AIC: IndirectCommandReadable>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<Ash, T>,
		indirect: impl MutOrSharedBuffer<Ash, [u32; 3], AIC>,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_mesh_graphics(pipeline, param)?;
			let device = &self.bindless.platform.extensions.mesh_shader();
			let indirect = indirect.inner_slot();
			device.cmd_draw_mesh_tasks_indirect(self.cmd, indirect.buffer, 0, 1, size_of::<[u32; 3]>() as u32);
			Ok(())
		}
	}
}
