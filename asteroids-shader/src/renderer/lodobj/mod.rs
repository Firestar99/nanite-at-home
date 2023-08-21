use spirv_std::glam::{Vec4, vec4};
use spirv_std::spirv;

use asteroids_common::renderer::lodobj::VertexInput;

#[spirv(vertex)]
pub fn bla_vs(
	vtx: VertexInput,
	#[spirv(position, invariant)] out_pos: &mut Vec4,
) {
	*out_pos = vec4(
		vtx.position.x,
		vtx.position.y,
		0.0,
		1.0,
	);
}

#[spirv(fragment)]
pub fn bla_fs(output: &mut Vec4) {
	*output = vec4(0.0, 0.0, 1.0, 1.0);
}
