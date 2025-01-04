use crate::renderer::compacting_alloc_buffer::CompactingAllocBufferWriting;
use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, RCDescExt};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, Recording, RecordingError};
use space_asset_rt::meshlet::scene::MeshletSceneCpu;
use space_asset_shader::meshlet::instance::MeshletInstance;
use space_engine_shader::renderer::meshlet::instance_cull::{Param, INSTANCE_CULL_WG_SIZE};
use std::sync::Arc;

pub struct InstanceCullCompute(BindlessComputePipeline<Param<'static>>);

impl InstanceCullCompute {
	pub fn new(bindless: &Arc<Bindless>) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::meshlet::instance_cull::instance_cull_compute::new(),
		)?))
	}

	#[profiling::function]
	pub fn dispatch(
		&self,
		cmd: &mut Recording<'_>,
		frame_context: &FrameContext,
		scene: &MeshletSceneCpu,
		alloc_buffer: &CompactingAllocBufferWriting<MeshletInstance>,
	) -> Result<(), RecordingError> {
		let groups_x = (scene.num_instances + INSTANCE_CULL_WG_SIZE - 1) / INSTANCE_CULL_WG_SIZE;
		cmd.dispatch(
			&self.0,
			[groups_x, 1, 1],
			Param {
				frame_data: frame_context.frame_data_desc,
				scene: scene.scene.to_transient(cmd),
				num_instances: scene.num_instances,
				compacting_alloc_buffer: alloc_buffer.to_writer()?,
			},
		)
	}
}
