use spirv_std::glam::{vec2, Vec4, vec4};
use spirv_std::spirv;

use space_engine_common::renderer::lodobj::VertexInput;

const VERTICES: [VertexInput; 4] = [
	VertexInput::new(vec2(-1., -1.)),
	VertexInput::new(vec2(-1., 1.)),
	VertexInput::new(vec2(1., 1.)),
	VertexInput::new(vec2(1., -1.)),
];

#[spirv(vertex)]
pub fn opaque_vs(
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position, invariant)] out_pos: &mut Vec4,
) {
	let vtx = &VERTICES[vertex_id as usize];
	*out_pos = vec4(
		vtx.position.x,
		vtx.position.y,
		0.0,
		1.0,
	);
}

#[spirv(fragment)]
pub fn opaque_fs(output: &mut Vec4) {
	*output = vec4(0.0, 0.0, 1.0, 1.0);
}
