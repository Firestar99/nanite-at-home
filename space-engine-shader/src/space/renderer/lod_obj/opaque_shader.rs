use glam::{Vec2, Vec4};
use spirv_std::{spirv, Image, Sampler};

use space_engine_common::space::renderer::frame_data::FrameData;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

#[spirv(vertex)]
pub fn opaque_vs(
	#[spirv(descriptor_set = 0, binding = 0, uniform)] frame_data: &FrameData,
	#[spirv(descriptor_set = 1, binding = 0, storage_buffer)] vertex_data: &[ModelVertex],
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position, invariant)] position: &mut Vec4,
	tex_coord: &mut Vec2,
) {
	let camera = frame_data.camera;
	let vertex_input = vertex_data[vertex_id as usize];

	let position_world = camera.transform.transform_point3(vertex_input.position.into());
	*position = camera.perspective * Vec4::from((position_world, 1.));
	*tex_coord = vertex_input.tex_coord;
}

#[spirv(fragment)]
pub fn opaque_fs(
	#[spirv(descriptor_set = 1, binding = 1)] image: &Image!(2D, type=f32, sampled),
	#[spirv(descriptor_set = 1, binding = 2)] sampler: &Sampler,
	tex_coord: Vec2,
	output: &mut Vec4,
) {
	*output = image.sample(*sampler, tex_coord);
}
