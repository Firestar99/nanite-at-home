use crate::renderer::lighting::lighting_pipeline::LightingPipeline;
use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use std::sync::Arc;
use vulkano::command_buffer::{CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer};
use vulkano::image::view::ImageView;
use vulkano::sync::GpuFuture;

pub struct LightingRenderTask {
	init: Arc<Init>,
	pipeline_lighting: LightingPipeline,
}

impl LightingRenderTask {
	pub fn new(init: &Arc<Init>) -> Self {
		let pipeline_lighting = LightingPipeline::new(init);
		Self {
			init: init.clone(),
			pipeline_lighting,
		}
	}

	#[profiling::function]
	pub fn record(
		&self,
		frame_context: &FrameContext,
		g_albedo: &Arc<ImageView>,
		g_normal: &Arc<ImageView>,
		g_rm: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		output_image: &Arc<ImageView>,
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
		self.pipeline_lighting.dispatch(
			frame_context,
			g_albedo,
			g_normal,
			g_rm,
			depth_image,
			output_image,
			&mut cmd,
		);
		let cmd = cmd.end().unwrap();

		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
