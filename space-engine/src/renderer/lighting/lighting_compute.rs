use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, Image2d, MutImage, Transient, TransientDesc};
use rust_gpu_bindless::pipeline::BindlessComputePipeline;
use rust_gpu_bindless::pipeline::{Recording, RecordingError};
use space_engine_shader::renderer::g_buffer::GBuffer;
use space_engine_shader::renderer::lighting::lighting_compute::{Param, LIGHTING_WG_SIZE};
use std::sync::Arc;

pub struct LightingCompute(BindlessComputePipeline<Param<'static>>);

impl LightingCompute {
	pub fn new(bindless: &Arc<Bindless>) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::lighting::lighting_compute::lighting_cs::new(),
		)?))
	}

	#[profiling::function]
	pub fn dispatch(
		&self,
		cmd: &mut Recording<'_>,
		frame_context: &FrameContext,
		g_buffer: GBuffer<Transient>,
		output_image: TransientDesc<MutImage<Image2d>>,
	) -> Result<(), RecordingError> {
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
				output_image,
			},
		)
	}
}
