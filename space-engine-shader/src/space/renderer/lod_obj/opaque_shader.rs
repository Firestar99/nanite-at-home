use crate::space::renderer::frame_data::FrameData;
use crate::space::renderer::model::model_vertex::ModelVertex;
use glam::{UVec3, Vec2, Vec4};
use spirv_std::arch::set_mesh_outputs_ext;
use spirv_std::image::Image2d;
use spirv_std::Sampler;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::bindless;
use vulkano_bindless_shaders::descriptor::descriptors::Descriptors;
use vulkano_bindless_shaders::descriptor::{Buffer, TransientDesc, ValidDesc};

#[derive(Copy, Clone)]
pub struct PushConstant<'a> {
	pub vertex_buffer: TransientDesc<'a, Buffer<[ModelVertex]>>,
	pub index_buffer: TransientDesc<'a, Buffer<[u32]>>,
	pub sampler: TransientDesc<'a, Sampler>,
}

unsafe impl bytemuck::Zeroable for PushConstant<'static> {}

unsafe impl bytemuck::AnyBitPattern for PushConstant<'static> {}

impl<'a> PushConstant<'a> {
	/// # Safety
	/// Should only be called by vulkano_bindless
	pub unsafe fn to_static(&self) -> PushConstant<'static> {
		unsafe {
			PushConstant {
				vertex_buffer: self.vertex_buffer.to_static(),
				index_buffer: self.index_buffer.to_static(),
				sampler: self.sampler.to_static(),
			}
		}
	}
}

const OUTPUT_VERTICES: usize = 3;
const OUTPUT_TRIANGLES: usize = 1;

#[bindless(mesh_ext(threads(1), output_vertices = 3, output_primitives_ext = 1, output_triangles_ext))]
pub fn opaque_mesh(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[spirv(descriptor_set = 1, binding = 0, uniform)] frame_data: &FrameData,
	#[spirv(push_constant)] push_constant: &PushConstant,
	#[spirv(global_invocation_id)] global_invocation_id: UVec3,
	#[spirv(primitive_triangle_indices_ext)] indices: &mut [UVec3; OUTPUT_TRIANGLES],
	#[spirv(position)] positions: &mut [Vec4; OUTPUT_VERTICES],
	vert_tex_coords: &mut [Vec2; OUTPUT_VERTICES],
	vert_texture: &mut [TransientDesc<Image2d>; OUTPUT_VERTICES],
) {
	unsafe {
		set_mesh_outputs_ext(OUTPUT_VERTICES as u32, OUTPUT_TRIANGLES as u32);
	}

	let camera = frame_data.camera;
	for i in 0..OUTPUT_VERTICES {
		let vertex_id = push_constant
			.index_buffer
			.access(descriptors)
			.load(global_invocation_id.x as usize * 3 + i);
		let vertex_input = push_constant.vertex_buffer.access(descriptors).load(vertex_id as usize);
		let position_world = camera.transform.transform_point3(vertex_input.position.into());
		positions[i] = camera.perspective * Vec4::from((position_world, 1.));
		vert_tex_coords[i] = vertex_input.tex_coord;
		vert_texture[i] = unsafe { vertex_input.tex_id.upgrade_unchecked() };
	}

	const_assert_eq!(OUTPUT_TRIANGLES, 1);
	indices[0] = UVec3::new(0, 1, 2);
}

#[bindless(fragment)]
pub fn opaque_fs(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[spirv(push_constant)] push_constant: &PushConstant,
	vert_tex_coords: Vec2,
	#[spirv(flat)] vert_texture: TransientDesc<Image2d>,
	output: &mut Vec4,
) {
	let image: &Image2d = vert_texture.access(descriptors);
	*output = image.sample(*push_constant.sampler.access(descriptors), vert_tex_coords);
	if output.w < 0.01 {
		spirv_std::arch::kill();
	}
}
