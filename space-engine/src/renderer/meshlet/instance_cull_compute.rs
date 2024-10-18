use crate::renderer::meshlet::meshlet_allocation_buffer::MeshletAllocationBuffer;
use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use rust_gpu_bindless::descriptor::RCDescExt;
use rust_gpu_bindless::pipeline::compute_pipeline::BindlessComputePipeline;
use space_asset_rt::meshlet::scene::MeshletSceneCpu;
use space_engine_shader::renderer::meshlet::instance_cull::{Params, INSTANCE_CULL_WG_SIZE};
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::{Pipeline, PipelineBindPoint, PipelineLayout};

pub struct InstanceCullCompute {
	pipeline: BindlessComputePipeline<Params<'static>>,
}

impl InstanceCullCompute {
	pub fn new(init: &Arc<Init>, alloc_buffer: &MeshletAllocationBuffer) -> Self {
		let pipeline = BindlessComputePipeline::new(
			init.bindless.clone(),
			crate::shader::renderer::meshlet::instance_cull::instance_cull_compute::new(),
			Some(init.pipeline_cache.deref().clone()),
			Some(
				PipelineLayout::new(
					init.device.clone(),
					PipelineLayoutCreateInfo {
						set_layouts: Vec::from([
							init.bindless.descriptor_set_layout.clone(),
							alloc_buffer.descriptor_set.layout().clone(),
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
		cmd: &mut RecordingCommandBuffer,
		alloc_buffer: &MeshletAllocationBuffer,
		scene: &MeshletSceneCpu,
	) {
		let groups_x = (scene.num_instances + INSTANCE_CULL_WG_SIZE - 1) / INSTANCE_CULL_WG_SIZE;
		unsafe {
			self.pipeline
				.dispatch(
					cmd,
					[groups_x, 1, 1],
					|cmd| {
						cmd.bind_descriptor_sets(
							PipelineBindPoint::Compute,
							self.pipeline.layout().clone(),
							1,
							alloc_buffer.descriptor_set.clone(),
						)
					},
					Params {
						frame_data: frame_context.frame_data_desc,
						scene: scene.scene.to_transient(frame_context.fif),
						num_instances: scene.num_instances,
					},
				)
				.unwrap();
		}
	}
}
