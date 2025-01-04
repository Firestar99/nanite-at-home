use crate::renderer::frame_context::FrameContext;
use rust_gpu_bindless::descriptor::{Bindless, Image2d, MutImage, Transient, TransientDesc};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, Recording, RecordingError};
use space_engine_shader::renderer::g_buffer::GBuffer;
use space_engine_shader::renderer::lighting::sky_shader::{Param, SKY_SHADER_WG_SIZE};
use std::sync::Arc;

pub struct SkyShaderCompute(BindlessComputePipeline<Param<'static>>);

impl SkyShaderCompute {
	pub fn new(bindless: &Arc<Bindless>) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::renderer::lighting::sky_shader::sky_shader_cs::new(),
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
		let image_size = frame_context.frame_data.viewport_size;
		let groups = [
			(image_size.x + SKY_SHADER_WG_SIZE.x - 1) / SKY_SHADER_WG_SIZE.x,
			(image_size.y + SKY_SHADER_WG_SIZE.y - 1) / SKY_SHADER_WG_SIZE.y,
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
