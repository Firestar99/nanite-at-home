use spirv_std::glam::{vec3a, Vec4, vec4};
use spirv_std::spirv;

use space_engine_common::space::renderer::frame_data::FrameData;
use space_engine_common::space::renderer::lod_obj::VertexInput;

const VERTICES: [VertexInput; 4] = [
	VertexInput::new(vec3a(-1., -1., 0.)),
	VertexInput::new(vec3a(-1., 1., 0.)),
	VertexInput::new(vec3a(1., 1., 0.)),
	VertexInput::new(vec3a(1., -1., 0.)),
];

#[spirv(vertex)]
pub fn opaque_vs(
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position, invariant)] out_pos: &mut Vec4,
	#[spirv(uniform, descriptor_set = 0, binding = 0)] frame_data: &FrameData,
) {
	let camera = frame_data.camera;
	let vtx = &VERTICES[vertex_id as usize];
	let position = camera.transform.transform_vector3a(vtx.position);
	*out_pos = (position, 1.).into();
}

#[spirv(fragment)]
pub fn opaque_fs(output: &mut Vec4) {
	*output = vec4(0.0, 0.0, 1.0, 1.0);
}
