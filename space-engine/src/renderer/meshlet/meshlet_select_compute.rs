use crate::renderer::compacting_alloc_buffer::{CompactingAllocBufferReading, CompactingAllocBufferWriting};
use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, RCDescExt};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, Recording, RecordingError};
use space_asset_rt::meshlet::scene::MeshletSceneCpu;
use space_engine_shader::renderer::meshlet::intermediate::{MeshletGroupInstance, MeshletInstance};
use space_engine_shader::renderer::meshlet::meshlet_select::Param;
use std::sync::Arc;

pub struct MeshletSelectCompute(BindlessComputePipeline<Param<'static>>);

impl MeshletSelectCompute {
	pub fn new(bindless: &Arc<Bindless>) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::meshlet::meshlet_select::meshlet_select_compute::new(),
		)?))
	}

	#[profiling::function]
	pub fn dispatch(
		&self,
		cmd: &mut Recording<'_>,
		frame_context: &FrameContext,
		scene: &MeshletSceneCpu,
		compacting_groups_in: &CompactingAllocBufferReading<MeshletGroupInstance>,
		compacting_instances_out: &CompactingAllocBufferWriting<MeshletInstance>,
	) -> Result<(), RecordingError> {
		cmd.dispatch_indirect(
			&self.0,
			compacting_groups_in.indirect_args(),
			Param {
				frame_data: frame_context.frame_data_desc,
				scene: scene.scene.to_transient(cmd),
				compacting_groups_in: compacting_groups_in.to_reader()?,
				compacting_instances_out: compacting_instances_out.to_writer()?,
			},
		)
	}
}
