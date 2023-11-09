use glam::{Vec4, vec4};
use spirv_std::spirv;

use space_engine_common::space::renderer::frame_data::FrameData;
use space_engine_common::space::renderer::lod_obj::VertexInput;

#[spirv(vertex)]
pub fn opaque_vs(
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position, invariant)] out_pos: &mut Vec4,
	#[spirv(descriptor_set = 0, binding = 0, uniform)] frame_data: &FrameData,
	#[spirv(descriptor_set = 1, binding = 0, storage_buffer)] vertex_input_buffer: &[VertexInput],
) {
	let camera = frame_data.camera;
	let vertex_input = vertex_input_buffer[vertex_id as usize];

	let position;
	{
		let p = camera.transform.transform_point3(vertex_input.position);
		position = camera.perspective * Vec4::from((p, 1.));
	}

	*out_pos = position;
}

#[spirv(fragment)]
pub fn opaque_fs(output: &mut Vec4) {
	*output = vec4(0.0, 0.0, 1.0, 1.0);
}
