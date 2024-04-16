use glam::{UVec3, Vec2, Vec4};
use space_engine_common::space::renderer::frame_data::FrameData;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;
use spirv_std::arch::set_mesh_outputs_ext;
use spirv_std::{spirv, Image, RuntimeArray, Sampler};
use static_assertions::const_assert_eq;
use vulkano_bindless_shaders::descriptor::buffer::{Buffer, BufferAccess};
use vulkano_bindless_shaders::descriptor::descriptors::Descriptors;
use vulkano_bindless_shaders::descriptor::TransientDesc;

const OUTPUT_VERTICES: usize = 3;
const OUTPUT_TRIANGLES: usize = 1;

pub struct PushConstant<'a> {
	pub vertex_buffer: TransientDesc<'a, Buffer<[ModelVertex]>>,
	pub index_buffer: TransientDesc<'a, Buffer<[u32]>>,
}

#[spirv(mesh_ext(threads(1), output_vertices = 3, output_primitives_ext = 1, output_triangles_ext))]
pub fn opaque_mesh<'a>(
	#[spirv(global_invocation_id)] global_invocation_id: UVec3,
	#[spirv(descriptor_set = 1, binding = 0, uniform)] frame_data: &FrameData,
	// #[spirv(descriptor_set = 1, binding = 0, storage_buffer)] vertex_data: &[ModelVertex],
	// #[spirv(descriptor_set = 1, binding = 1, storage_buffer)] index_data: &[u32],
	#[spirv(descriptor_set = 0, binding = 0, storage_buffer)] buffer_data: &mut [&mut [u32]],
	#[spirv(primitive_triangle_indices_ext)] indices: &mut [UVec3; OUTPUT_TRIANGLES],
	#[spirv(position)] positions: &mut [Vec4; OUTPUT_VERTICES],
	#[spirv(push_constant)] push_constant: &PushConstant,
	tex_coords: &mut [Vec2; OUTPUT_VERTICES],
	tex_ids: &mut [u32; OUTPUT_VERTICES],
) {
	let mut descriptors = Descriptors::new(buffer_data);

	unsafe {
		set_mesh_outputs_ext(OUTPUT_VERTICES as u32, OUTPUT_TRIANGLES as u32);
	}

	let camera = frame_data.camera;
	// let vertex_data = *buffer_data.index_mut(push_constant.vertex_buffer.id() as usize);
	// let vertex_data: &[ModelVertex] = bytemuck::cast_slice(vertex_data);
	// let vertex_data = {
	// 	let a = vertex_data;
	// 	let new_len = core::mem::size_of_val(a) / size_of::<ModelVertex>();
	// 	unsafe { core::slice::from_raw_parts(a.as_ptr() as *const ModelVertex, new_len) }
	// };

	// let vertex_data: BufferSlice<[ModelVertex]> = BufferSlice::new(vertex_data);

	// let index_data = buffer_data.index_mut(push_constant.index_buffer.id() as usize);

	// let vertex_data: &[ModelVertex] = bytemuck::cast_slice(unsafe {
	// 	buffer_data.index(push_constant.vertex_buffer.id() as usize)
	// });
	// let index_data = unsafe {
	// 	buffer_data.index(push_constant.index_buffer.id() as usize)
	// };

	for i in 0..OUTPUT_VERTICES {
		let vertex_id = push_constant
			.index_buffer
			.access(&mut descriptors)
			.load(global_invocation_id.x as usize * 3 + i);
		let vertex_input = push_constant
			.vertex_buffer
			.access(&mut descriptors)
			.load(vertex_id as usize);
		let position_world = camera.transform.transform_point3(vertex_input.position.into());
		positions[i] = camera.perspective * Vec4::from((position_world, 1.));
		tex_coords[i] = vertex_input.tex_coord;
		tex_ids[i] = vertex_input.tex_id.0;
	}

	const_assert_eq!(OUTPUT_TRIANGLES, 1);
	indices[0] = UVec3::new(0, 1, 2);
}

#[spirv(fragment)]
pub fn opaque_fs(
	#[spirv(descriptor_set = 2, binding = 2)] sampler: &Sampler,
	#[spirv(descriptor_set = 3, binding = 0)] images: &RuntimeArray<Image!(2D, type=f32, sampled)>,
	tex_coords: Vec2,
	#[spirv(flat)] tex_ids: u32,
	output: &mut Vec4,
) {
	let image = unsafe { images.index(tex_ids as usize) };
	*output = image.sample(*sampler, tex_coords);
	if output.w < 0.01 {
		spirv_std::arch::kill();
	}
}
