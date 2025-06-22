use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, Image2d, Transient};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, MutImageAccess, StorageReadWrite};
use rust_gpu_bindless::pipeline::{Recording, RecordingError};
use space_engine_shader::renderer::g_buffer::GBuffer;
use space_engine_shader::renderer::lighting::lighting_compute::{Param, LIGHTING_WG_SIZE};

pub struct LightingCompute(BindlessComputePipeline<Param<'static>>);

impl LightingCompute {
	pub fn new(bindless: &Bindless) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::lighting::lighting_compute::lighting_cs::new(),
		)?))
	}

	pub fn dispatch(
		&self,
		cmd: &mut Recording<'_>,
		frame_context: &FrameContext,
		g_buffer: GBuffer<Transient>,
		output_image: &MutImageAccess<'_, Image2d, StorageReadWrite>,
	) -> Result<(), RecordingError> {
		profiling::function_scope!();
		let image_size = frame_context.frame_data.camera.viewport_size;
		let groups = [
			(image_size.x + LIGHTING_WG_SIZE - 1) / LIGHTING_WG_SIZE,
			image_size.y,
			1,
		];
		cmd.dispatch(
			&self.0,
			groups,
			Param {
				frame_data: frame_context.frame_data_desc,
				g_buffer,
				output_image: output_image.to_mut_transient(),
			},
		)
	}
}
