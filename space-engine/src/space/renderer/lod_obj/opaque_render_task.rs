use std::sync::Arc;

use vulkano::command_buffer::{AutoCommandBufferBuilder, RenderingAttachmentInfo, RenderingInfo, SubpassContents};
use vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit;
use vulkano::format::{ClearValue, Format};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

use crate::space::renderer::lod_obj::opaque_draw::OpaqueDrawPipeline;
use crate::space::renderer::render_graph::context::{FrameContext, RenderContext};

pub struct OpaqueRenderTask {
	pipeline_opaque: OpaqueDrawPipeline,
}

impl OpaqueRenderTask {
	pub fn new<'a>(context: &Arc<RenderContext>, format: Format) -> Self {
		Self {
			pipeline_opaque: OpaqueDrawPipeline::new(context, format),
		}
	}

	pub fn record(&self, frame_context: &FrameContext, future: impl GpuFuture) -> impl GpuFuture {
		let graphics = &frame_context.render_context.init.queues.client.graphics_main;
		let mut cmd = AutoCommandBufferBuilder::primary(&frame_context.render_context.init.cmdbuffer_allocator, graphics.queue_family_index(), OneTimeSubmit).unwrap();
		cmd.begin_rendering(RenderingInfo {
			color_attachments: vec![Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Float([0.0f32; 4])),
				..RenderingAttachmentInfo::image_view(frame_context.output_image.clone())
			})],
			contents: SubpassContents::Inline,
			..RenderingInfo::default()
		}).unwrap();
		self.pipeline_opaque.draw(frame_context, &mut cmd);
		cmd.end_rendering().unwrap();

		future.then_execute(graphics.clone(), cmd.build().unwrap()).unwrap()
	}
}
