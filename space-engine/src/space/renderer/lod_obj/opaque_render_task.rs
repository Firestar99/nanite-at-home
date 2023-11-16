use glam::vec3;
use std::sync::Arc;

use vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit;
use vulkano::command_buffer::{AutoCommandBufferBuilder, RenderingAttachmentInfo, RenderingInfo, SubpassContents};
use vulkano::format::{ClearValue, Format};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

use space_engine_common::space::renderer::lod_obj::VertexInput;

use crate::space::renderer::lod_obj::opaque_draw::OpaqueDrawPipeline;
use crate::space::renderer::lod_obj::opaque_model::OpaqueModel;
use crate::space::renderer::render_graph::context::{FrameContext, RenderContext};

pub struct OpaqueRenderTask {
	pipeline_opaque: OpaqueDrawPipeline,
	opaque_model: OpaqueModel,
}

const MODEL_VERTEX_INPUT: [VertexInput; 4] = [
	VertexInput::new(vec3(-1., -1., 0.)),
	VertexInput::new(vec3(-1., 1., 0.)),
	VertexInput::new(vec3(1., 1., 0.)),
	VertexInput::new(vec3(1., -1., 0.)),
];

impl OpaqueRenderTask {
	pub fn new<'a>(context: &Arc<RenderContext>, format: Format) -> Self {
		let pipeline_opaque = OpaqueDrawPipeline::new(context, format);
		let opaque_model = OpaqueModel::new(&context, &pipeline_opaque, MODEL_VERTEX_INPUT.iter().copied());
		Self {
			pipeline_opaque,
			opaque_model,
		}
	}

	pub fn record(&self, frame_context: &FrameContext, future: impl GpuFuture) -> impl GpuFuture {
		let graphics = &frame_context.render_context.init.queues.client.graphics_main;
		let mut cmd = AutoCommandBufferBuilder::primary(
			&frame_context.render_context.init.cmd_buffer_allocator,
			graphics.queue_family_index(),
			OneTimeSubmit,
		)
		.unwrap();
		cmd.begin_rendering(RenderingInfo {
			color_attachments: vec![Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Float([0.0f32; 4])),
				..RenderingAttachmentInfo::image_view(frame_context.output_image.clone())
			})],
			contents: SubpassContents::Inline,
			..RenderingInfo::default()
		})
		.unwrap();
		self.pipeline_opaque.draw(frame_context, &mut cmd, &self.opaque_model);
		cmd.end_rendering().unwrap();

		future.then_execute(graphics.clone(), cmd.build().unwrap()).unwrap()
	}
}
