use glam::{Vec2, Vec4};
use spirv_std::{spirv, Image, RuntimeArray, Sampler};

use space_engine_common::space::renderer::frame_data::FrameData;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

#[spirv(vertex)]
pub fn opaque_vs(
	#[spirv(descriptor_set = 0, binding = 0, uniform)] frame_data: &FrameData,
	#[spirv(descriptor_set = 1, binding = 0, storage_buffer)] vertex_data: &[ModelVertex],
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position, invariant)] position: &mut Vec4,
	tex_coord: &mut Vec2,
	tex_id: &mut u32,
) {
	let camera = frame_data.camera;
	let vertex_input = vertex_data[vertex_id as usize];

	let position_world = camera.transform.transform_point3(vertex_input.position.into());
	*position = camera.perspective * Vec4::from((position_world, 1.));
	*tex_coord = vertex_input.tex_coord;
	*tex_id = vertex_input.tex_id.0;
}

#[spirv(fragment)]
pub fn opaque_fs(
	#[spirv(descriptor_set = 1, binding = 1)] sampler: &Sampler,
	#[spirv(descriptor_set = 2, binding = 0)] images: &RuntimeArray<Image!(2D, type=f32, sampled)>,
	tex_coord: Vec2,
	#[spirv(flat)] tex_id: u32,
	output: &mut Vec4,
) {
	let image = unsafe { images.index(tex_id as usize) };
	*output = image.sample(*sampler, tex_coord);
}
