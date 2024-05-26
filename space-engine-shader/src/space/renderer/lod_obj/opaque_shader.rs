use crate::space::renderer::frame_data::FrameData;
use crate::space::renderer::model::gpu_model::OpaqueGpuModel;
use glam::{UVec3, Vec2, Vec4};
use spirv_std::arch::{emit_mesh_tasks_ext_payload, set_mesh_outputs_ext};
use spirv_std::image::Image2d;
use spirv_std::Sampler;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::{bindless, DescStruct};
use vulkano_bindless_shaders::descriptor::descriptors::Descriptors;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::{Buffer, TransientDesc, ValidDesc};

#[derive(Copy, Clone, DescStruct)]
pub struct Params<'a> {
	pub models: TransientDesc<'a, Buffer<[OpaqueGpuModel]>>,
	pub sampler: TransientDesc<'a, Sampler>,
}

#[derive(Copy, Clone)]
pub struct Payload {
	pub model_offset: usize,
}

#[bindless(task_ext(threads(1)))]
pub fn opaque_task(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	#[spirv(global_invocation_id)] global_invocation_id: UVec3,
	#[spirv(task_payload_workgroup_ext)] payload: &mut Payload,
) {
	let global_id = global_invocation_id.x as usize;
	// Safety: cannot use load() as a panic before emit_mesh_tasks_ext() mispiles
	let model = unsafe { param.models.access(descriptors).load_unchecked(global_id) };
	payload.model_offset = global_id;

	unsafe {
		emit_mesh_tasks_ext_payload(model.triangle_count, 1, 1, payload);
	}
}

const OUTPUT_VERTICES: usize = 3;
const OUTPUT_TRIANGLES: usize = 1;

#[bindless(mesh_ext(threads(1), output_vertices = 3, output_primitives_ext = 1, output_triangles_ext))]
pub fn opaque_mesh(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[spirv(descriptor_set = 1, binding = 0, uniform)] frame_data: &FrameData,
	#[bindless(param_constants)] param: &Params<'static>,
	#[spirv(task_payload_workgroup_ext)] payload: &Payload,
	#[spirv(global_invocation_id)] global_invocation_id: UVec3,
	#[spirv(primitive_triangle_indices_ext)] indices: &mut [UVec3; OUTPUT_TRIANGLES],
	#[spirv(position)] positions: &mut [Vec4; OUTPUT_VERTICES],
	vert_tex_coords: &mut [Vec2; OUTPUT_VERTICES],
	vert_texture: &mut [StrongDesc<Image2d>; OUTPUT_VERTICES],
) {
	unsafe {
		set_mesh_outputs_ext(OUTPUT_VERTICES as u32, OUTPUT_TRIANGLES as u32);
	}

	let model = param.models.access(descriptors).load(payload.model_offset);
	let index_buffer = model.index_buffer.access(descriptors);
	let vertex_buffer = model.vertex_buffer.access(descriptors);

	let camera = frame_data.camera;
	for i in 0..OUTPUT_VERTICES {
		let vertex_id = index_buffer.load(global_invocation_id.x as usize * 3 + i);
		let vertex_input = vertex_buffer.load(vertex_id as usize);
		let position_world = camera.transform.transform_point3(vertex_input.position.into());
		positions[i] = camera.perspective * Vec4::from((position_world, 1.));
		vert_tex_coords[i] = vertex_input.tex_coord;
		vert_texture[i] = vertex_input.tex_id;
	}

	const_assert_eq!(OUTPUT_TRIANGLES, 1);
	indices[0] = UVec3::new(0, 1, 2);
}

#[bindless(fragment())]
pub fn opaque_fs(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	vert_tex_coords: Vec2,
	#[spirv(flat)] vert_texture: StrongDesc<Image2d>,
	output: &mut Vec4,
) {
	let image: &Image2d = vert_texture.access(descriptors);
	*output = image.sample(*param.sampler.access(descriptors), vert_tex_coords);
	if output.w < 0.01 {
		spirv_std::arch::kill();
	}
}
