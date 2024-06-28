use crate::space::renderer::meshlet::mesh_pipeline::MeshDrawPipeline;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;
use parking_lot::Mutex;
use space_asset::meshlet::scene::MeshletSceneCpu;
use std::sync::Arc;
use vulkano::command_buffer::{
	CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer, RenderingAttachmentInfo,
	RenderingInfo, SubpassContents,
};
use vulkano::format::{ClearValue, Format};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::GpuFuture;

pub struct MeshletRenderTask {
	init: Arc<Init>,
	pipeline_mesh: MeshDrawPipeline,
	pub scenes: Mutex<Vec<Arc<MeshletSceneCpu>>>,
}

impl MeshletRenderTask {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline_mesh = MeshDrawPipeline::new(init, format_color, format_depth);

		Self {
			init: init.clone(),
			pipeline_mesh,
			scenes: Mutex::new(Vec::new()),
		}
	}

	#[profiling::function]
	pub fn record(
		&self,
		frame_context: &FrameContext,
		output_image: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		future: impl GpuFuture,
	) -> impl GpuFuture {
		let init = &self.init;
		let graphics = &init.queues.client.graphics_main;

		let mut cmd = RecordingCommandBuffer::new(
			init.cmd_buffer_allocator.clone(),
			graphics.queue_family_index(),
			CommandBufferLevel::Primary,
			CommandBufferBeginInfo {
				usage: CommandBufferUsage::OneTimeSubmit,
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
				clear_value: Some(ClearValue::Depth(1.)),
				..RenderingAttachmentInfo::image_view(depth_image.clone())
			}),
			contents: SubpassContents::Inline,
			..RenderingInfo::default()
		})
		.unwrap();
		let scenes = self.scenes.lock().clone();
		for (_id, scene) in scenes.iter().enumerate() {
			profiling::scope!("draw scene", _id.to_string());
			for mesh2instance in &scene.mesh2instances {
				self.pipeline_mesh.draw(frame_context, &mut cmd, mesh2instance);
			}
		}
		cmd.end_rendering().unwrap();
		let cmd = cmd.end().unwrap();

		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
