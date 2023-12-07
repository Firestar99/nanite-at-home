use std::sync::Arc;

use glam::vec3;
use vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit;
use vulkano::command_buffer::{AutoCommandBufferBuilder, RenderingAttachmentInfo, RenderingInfo, SubpassContents};
use vulkano::format::{ClearValue, Format};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::lod_obj::opaque_draw::OpaqueDrawPipeline;
use crate::space::renderer::model::model::OpaqueModel;
use crate::space::renderer::model::model_descriptor_set::ModelDescriptorSetLayout;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

pub struct OpaqueRenderTask {
	pipeline_opaque: OpaqueDrawPipeline,
	opaque_model: OpaqueModel,
}

const MODEL_VERTEX_INPUT: [ModelVertex; 4] = [
	ModelVertex::new(vec3(-1., -1., 0.)),
	ModelVertex::new(vec3(-1., 1., 0.)),
	ModelVertex::new(vec3(1., 1., 0.)),
	ModelVertex::new(vec3(1., -1., 0.)),
];

impl OpaqueRenderTask {
	pub fn new(init: &Arc<Init>, format: Format) -> Self {
		let pipeline_opaque = OpaqueDrawPipeline::new(&init, format);
		let model_descriptor_set_layout = ModelDescriptorSetLayout::new(init);
		let opaque_model = OpaqueModel::new(&init, &model_descriptor_set_layout, MODEL_VERTEX_INPUT.iter().copied());
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
