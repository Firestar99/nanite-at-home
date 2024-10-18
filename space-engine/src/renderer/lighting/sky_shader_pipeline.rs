use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use rust_gpu_bindless::pipeline::compute_pipeline::BindlessComputePipeline;
use space_engine_shader::renderer::lighting::sky_shader::{Params, SKY_SHADER_WG_SIZE};
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::DescriptorSet;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::PipelineBindPoint;
use vulkano::pipeline::{Pipeline, PipelineLayout};

pub struct SkyShaderPipeline {
	pipeline: BindlessComputePipeline<Params<'static>>,
}

impl SkyShaderPipeline {
	pub fn new(init: &Arc<Init>, image_descriptor_set_layout: &Arc<DescriptorSetLayout>) -> Self {
		let pipeline = BindlessComputePipeline::new(
			init.bindless.clone(),
			crate::shader::renderer::lighting::sky_shader::sky_shader_cs::new(),
			Some(init.pipeline_cache.deref().clone()),
			Some(
				PipelineLayout::new(
					init.device.clone(),
					PipelineLayoutCreateInfo {
						set_layouts: Vec::from([
							init.bindless.descriptor_set_layout.clone(),
							image_descriptor_set_layout.clone(),
						]),
						push_constant_ranges: init.bindless.get_push_constant::<Params<'static>>(),
						..PipelineLayoutCreateInfo::default()
					},
				)
				.unwrap(),
			),
		)
		.unwrap();

		Self { pipeline }
	}

	#[profiling::function]
	pub fn dispatch(
		&self,
		frame_context: &FrameContext,
		image_descriptor: Arc<DescriptorSet>,
		cmd: &mut RecordingCommandBuffer,
	) {
		unsafe {
			let image_size = frame_context.frame_data.viewport_size;
			let groups = [
				(image_size.x + SKY_SHADER_WG_SIZE.x - 1) / SKY_SHADER_WG_SIZE.x,
				(image_size.y + SKY_SHADER_WG_SIZE.y - 1) / SKY_SHADER_WG_SIZE.y,
				1,
			];
			self.pipeline
				.dispatch(
					cmd,
					groups,
					|cmd| {
						cmd.bind_descriptor_sets(
							PipelineBindPoint::Compute,
							self.pipeline.layout().clone(),
							1,
							image_descriptor,
						)
					},
					Params {
						frame_data: frame_context.frame_data_desc,
					},
				)
				.unwrap();
		}
	}
}
