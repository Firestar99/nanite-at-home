use spirv_std::glam::{Vec4, vec4, Vec2};
use spirv_std::spirv;

#[spirv(fragment)]
pub fn bla_fs(output: &mut Vec4) {
	*output = vec4(0.0, 0.0, 1.0, 1.0);
}

#[spirv(vertex)]
pub fn bla_vs(
	position: Vec2,
	#[spirv(position, invariant)] out_pos: &mut Vec4,
) {
	*out_pos = vec4(
		position.x,
		position.y,
		0.0,
		1.0,
	);
}
