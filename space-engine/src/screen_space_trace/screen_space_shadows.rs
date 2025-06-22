use rust_gpu_bindless::descriptor::{Bindless, Image2d, ImageDescExt, TransientDesc};
use rust_gpu_bindless::pipeline::{BindlessComputePipeline, MutImageAccess, StorageReadWrite};
use rust_gpu_bindless::pipeline::{Recording, RecordingError};
use rust_gpu_bindless_shaders::descriptor::Image;
use rust_gpu_bindless_shaders::spirv_std::glam::{UVec2, Vec3, Vec3Swizzles};
use space_engine_shader::screen_space_trace::directional_shadows::{Param, DIRECTIONAL_SHADOWS_WG_SIZE};
use space_engine_shader::screen_space_trace::major_axis::TraceDirection;

#[derive(Copy, Clone)]
pub struct DirectionalShadowConfig<'a> {
	/// Depth image to trace in
	pub depth_image: TransientDesc<'a, Image<Image2d>>,
	/// Output image to write trace lengths into channel r.
	pub out_image: &'a MutImageAccess<'a, Image2d, StorageReadWrite>,
	/// Length of the screen space ray in pixels
	pub trace_length: u32,
	/// In meters of camera space
	pub object_thickness: f32,
	/// Direction in which to trace
	pub trace_direction: Vec3,
}

pub struct TraceDirectionalShadowsCompute(BindlessComputePipeline<Param<'static>>);

impl TraceDirectionalShadowsCompute {
	pub fn new(bindless: &Bindless) -> anyhow::Result<Self> {
		Ok(Self(bindless.create_compute_pipeline(
			crate::shader::screen_space_trace::directional_shadows::directional_shadows::new(),
		)?))
	}

	pub fn dispatch(&self, cmd: &mut Recording<'_>, config: DirectionalShadowConfig) -> Result<(), RecordingError> {
		profiling::function_scope!();

		let trace_direction = TraceDirection::new(config.trace_direction.xy());
		let image_size = UVec2::from(config.out_image.extent());
		let groups = trace_direction.major_dir().abs().as_uvec2() * image_size
			+ trace_direction.minor_dir().abs().as_uvec2() * image_size;
		let groups = [
			(groups.x + DIRECTIONAL_SHADOWS_WG_SIZE - 1) / DIRECTIONAL_SHADOWS_WG_SIZE,
			groups.y,
			1,
		];

		cmd.dispatch(
			&self.0,
			groups,
			Param {
				depth_image: config.depth_image,
				out_image: config.out_image.to_mut_transient(),
				image_size,
				trace_direction,
				trace_direction_z: config.trace_direction.z,
				trace_length: config.trace_length,
				object_thickness: config.object_thickness,
			},
		)
	}
}
