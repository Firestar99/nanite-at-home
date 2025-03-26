use crate::renderer::compacting_alloc_buffer::CompactingAllocBufferWriting;
use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, RCDescExt};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, Recording, RecordingError};
use space_asset_rt::meshlet::scene::InstancedMeshletSceneCpu;
use space_engine_shader::renderer::meshlet::instance_cull::Param;
use space_engine_shader::renderer::meshlet::intermediate::MeshletGroupInstance;

pub struct InstanceCullCompute(BindlessComputePipeline<Param<'static>>);

impl InstanceCullCompute {
	pub fn new(bindless: &Bindless) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::meshlet::instance_cull::instance_cull_compute::new(),
		)?))
	}

	pub fn dispatch(
		&self,
		cmd: &mut Recording<'_>,
		frame_context: &FrameContext,
		scene: &InstancedMeshletSceneCpu,
		alloc_buffer: &CompactingAllocBufferWriting<MeshletGroupInstance>,
	) -> Result<(), RecordingError> {
		profiling::function_scope!();
		let groups_x = scene.num_instances;
		cmd.dispatch(
			&self.0,
			[groups_x, 1, 1],
			Param {
				frame_data: frame_context.frame_data_desc,
				scene: scene.scene.to_transient(cmd),
				compacting_groups_out: alloc_buffer.to_writer()?,
			},
		)
	}
}
