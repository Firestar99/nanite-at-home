use crate::pipeline::access_type::{ColorAttachment, DepthStencilAttachment};
use crate::pipeline::graphics_pipeline::BindlessGraphicsPipeline;
use crate::pipeline::mesh_graphics_pipeline::BindlessMeshGraphicsPipeline;
use crate::pipeline::recording::HasResourceContext;
use crate::pipeline::rendering::{RenderPassFormat, RenderingAttachment};
use crate::platform::ash::bindless_pipeline::AshPipeline;
use crate::platform::ash::{Ash, AshRecordingContext, AshRecordingError, AshRecordingResourceContext};
use crate::platform::RenderingContext;
use ash::vk::{
	Extent2D, ImageLayout, Offset2D, PipelineBindPoint, Rect2D, RenderingAttachmentInfo, RenderingInfo, Viewport,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::TransientAccess;
use smallvec::SmallVec;
use std::ops::{Deref, DerefMut};

pub struct AshRenderingContext<'a, 'b> {
	recording: &'b mut AshRecordingContext<'a>,
	graphics_bind_descriptors: bool,
}

impl<'a, 'b> Deref for AshRenderingContext<'a, 'b> {
	type Target = AshRecordingContext<'a>;

	fn deref(&self) -> &Self::Target {
		&self.recording
	}
}

impl<'a, 'b> DerefMut for AshRenderingContext<'a, 'b> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.recording
	}
}

impl<'a, 'b> AshRenderingContext<'a, 'b> {
	pub unsafe fn new(recording: &'b mut AshRecordingContext<'a>) -> Self {
		Self {
			recording,
			graphics_bind_descriptors: true,
		}
	}

	#[inline]
	pub unsafe fn ash_bind_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		param: T,
	) -> Result<(), AshRecordingError> {
		self.ash_bind_any_graphics(&pipeline.inner().0, param)
	}

	#[inline]
	pub unsafe fn ash_bind_mesh_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessMeshGraphicsPipeline<Ash, T>,
		param: T,
	) -> Result<(), AshRecordingError> {
		self.ash_bind_any_graphics(&pipeline.inner().0, param)
	}

	pub unsafe fn ash_bind_any_graphics<T: BufferStruct>(
		&mut self,
		pipeline: &AshPipeline,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_must_not_flush()?;
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

unsafe impl<'a, 'b> TransientAccess<'a> for AshRenderingContext<'a, 'b> {}

unsafe impl<'a, 'b> HasResourceContext<'a, Ash> for AshRenderingContext<'a, 'b> {
	unsafe fn resource_context(&self) -> &'a AshRecordingResourceContext {
		self.recording.resource_context()
	}
}

unsafe impl<'a, 'b> RenderingContext<'a, 'b, Ash> for AshRenderingContext<'a, 'b> {
	unsafe fn begin_rendering(
		recording: &'b mut AshRecordingContext<'a>,
		_format: RenderPassFormat,
		render_area: [u32; 2],
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
							width: render_area[0],
							height: render_area[1],
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
			device.cmd_set_viewport(
				recording.cmd,
				0,
				&[Viewport {
					x: 0.0,
					y: 0.0,
					width: render_area[0] as f32,
					height: render_area[1] as f32,
					min_depth: 0.0,
					max_depth: 1.0,
				}],
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

	unsafe fn draw<T: BufferStruct>(
		&mut self,
		pipeline: &BindlessGraphicsPipeline<Ash, T>,
		vertex_count: u32,
		instance_count: u32,
		first_vertex: u32,
		first_instance: u32,
		param: T,
	) -> Result<(), AshRecordingError> {
		unsafe {
			self.ash_bind_graphics(pipeline, param)?;
			let device = &self.bindless.platform.device;
			device.cmd_draw(self.cmd, vertex_count, instance_count, first_vertex, first_instance);
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
			let device = &self.bindless.platform.extensions.ext_mesh_shader();
			device.cmd_draw_mesh_tasks(self.cmd, group_counts[0], group_counts[1], group_counts[2]);
			Ok(())
		}
	}
}
