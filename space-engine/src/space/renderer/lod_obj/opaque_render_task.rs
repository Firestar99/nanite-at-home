use crate::space::renderer::lod_obj::opaque_draw::OpaqueDrawPipeline;
use crate::space::renderer::model::model::OpaqueModel;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::command_buffer::CommandBufferLevel::Primary;
use vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, RecordingCommandBuffer, RenderingAttachmentInfo, RenderingInfo, SubpassContents,
};
use vulkano::format::{ClearValue, Format};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

pub struct OpaqueRenderTask {
	init: Arc<Init>,
	pipeline_opaque: OpaqueDrawPipeline,
	pub models: Mutex<Vec<OpaqueModel>>,
}

impl OpaqueRenderTask {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline_opaque = OpaqueDrawPipeline::new(&init, format_color, format_depth);
		Self {
			init: init.clone(),
			pipeline_opaque,
			models: Mutex::new(Vec::new()),
		}
	}

	pub fn record(
		&self,
		frame_context: &FrameContext,
		output_image: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		future: impl GpuFuture,
	) -> impl GpuFuture {
		let graphics = &frame_context.render_context.init.queues.client.graphics_main;
		let mut cmd = RecordingCommandBuffer::new(
			frame_context.render_context.init.cmd_buffer_allocator.clone(),
			graphics.queue_family_index(),
			Primary,
			CommandBufferBeginInfo {
				usage: OneTimeSubmit,
				..CommandBufferBeginInfo::default()
			},
		)
		.unwrap();
		cmd.begin_rendering(RenderingInfo {
			color_attachments: vec![Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Float([0.0f32; 4])),
				..RenderingAttachmentInfo::image_view(output_image.clone())
			})],
			depth_attachment: Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				// FIXME: what is the clear value, 1 or +INF?
				clear_value: Some(ClearValue::Depth(1.)),
				..RenderingAttachmentInfo::image_view(depth_image.clone())
			}),
			contents: SubpassContents::Inline,
			..RenderingInfo::default()
		})
		.unwrap();
		for model in &*self.models.lock() {
			self.pipeline_opaque.draw(frame_context, &mut cmd, model);
		}
		cmd.end_rendering().unwrap();
		let cmd = cmd.end().unwrap();

		self.init.descriptors.flush();
		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
