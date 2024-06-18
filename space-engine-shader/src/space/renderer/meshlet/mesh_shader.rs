#![allow(warnings)]

use crate::space::renderer::frame_data::FrameData;
use glam::{vec3, UVec3, Vec3, Vec4};
use space_asset_shader::meshlet::mesh::{MeshletMesh, MeshletVertex};
use space_asset_shader::meshlet::scene::MeshletInstance;
use space_asset_shader::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use spirv_std::arch::{
	atomic_i_add, emit_mesh_tasks_ext_payload, set_mesh_outputs_ext, subgroup_non_uniform_ballot,
	subgroup_non_uniform_ballot_bit_count, subgroup_non_uniform_elect, workgroup_memory_barrier_with_group_sync,
	GroupOperation, IndexUnchecked,
};
use spirv_std::memory::{Scope, Semantics};
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::{bindless, DescStruct};
use vulkano_bindless_shaders::descriptor::reference::Strong;
use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[derive(Copy, Clone, DescStruct)]
pub struct Params<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub mesh: TransientDesc<'a, Buffer<MeshletMesh<Strong>>>,
	pub instances: TransientDesc<'a, Buffer<[MeshletInstance<Strong>]>>,
}

#[derive(Copy, Clone, DescStruct)]
struct Payload {
	pub instance_id: u32,
	pub meshlet_offset: u32,
	// pub draw_ballot: [u32; TASK_DRAW_BALLOT_SIZE],
}

pub const TASK_WG_SIZE: u32 = 32;
// const TASK_DRAW_BALLOT_SIZE: usize = (TASK_WG_SIZE+31) / 32;

const_assert_eq!(TASK_WG_SIZE, 32);
#[bindless(task_ext(threads(32)))]
pub fn meshlet_task(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
	#[spirv(num_subgroups)] sg_num: u32,
	#[spirv(workgroup)] wg_lds: &mut [u32; 1],
	#[spirv(task_payload_workgroup_ext)] payload: &mut Payload,
) {
	// Safety: bounds checks in task shaders are broken
	unsafe {
		let wg_meshlet_offset = wg_id.x * TASK_WG_SIZE;
		let wg_instance_id = wg_id.y;
		let meshlet_id = wg_meshlet_offset + inv_id.x;

		let mesh = param.mesh.access(descriptors).load();
		let instance = param
			.instances
			.access(descriptors)
			.load_unchecked(wg_instance_id as usize);

		let draw = draw_meshlet(descriptors, instance, mesh, meshlet_id);

		let draw_ballot = subgroup_non_uniform_ballot(draw);
		let draw_subgroup_count =
			subgroup_non_uniform_ballot_bit_count::<{ GroupOperation::Reduce as u32 }>(draw_ballot);

		let draw_count = if sg_num == 1 {
			draw_subgroup_count
		} else {
			let draw_count_shared = &mut wg_lds[0];
			*draw_count_shared = 0;
			workgroup_memory_barrier_with_group_sync();
			if subgroup_non_uniform_elect() {
				atomic_i_add::<_, { Scope::Workgroup as u32 }, { Semantics::WORKGROUP_MEMORY.bits() }>(
					draw_count_shared,
					draw_subgroup_count,
				);
			}
			workgroup_memory_barrier_with_group_sync();
			*draw_count_shared
		};

		*payload = Payload {
			instance_id: wg_instance_id,
			meshlet_offset: wg_meshlet_offset,
			// TODO impl ballot forwarding, this is only fine as long as we only cull for num_meshlets
			// draw_ballot: core::array::from_fn(|i| draw_ballot.0[i]),
		};

		emit_mesh_tasks_ext_payload(draw_count, 1, 1, payload);
	}
}

fn draw_meshlet(
	_descriptors: &Descriptors,
	_instance: MeshletInstance<Strong>,
	mesh: MeshletMesh<Strong>,
	meshlet_id: u32,
) -> bool {
	// Safety: bounds checks in task shaders are broken
	// unsafe {
	if meshlet_id >= mesh.num_meshlets {
		return false;
	}

	// let meshlet = mesh.meshlet_unchecked(descriptors, meshlet_id);

	return true;
	// }
}

pub const MESH_WG_SIZE: usize = 32;

const_assert_eq!(MESH_WG_SIZE, 32);
const_assert_eq!(MESHLET_MAX_VERTICES, 64);
const_assert_eq!(MESHLET_MAX_TRIANGLES, 126);
#[bindless(mesh_ext(threads(32), output_vertices = 64, output_primitives_ext = 126, output_triangles_ext))]
pub fn meshlet_mesh(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
	#[spirv(task_payload_workgroup_ext)] payload: &Payload,
	#[spirv(primitive_triangle_indices_ext)] prim_indices: &mut [UVec3; MESHLET_MAX_TRIANGLES as usize],
	#[spirv(position)] vtx_positions: &mut [Vec4; MESHLET_MAX_VERTICES as usize],
	vtx_meshlet_id: &mut [u32; MESHLET_MAX_TRIANGLES as usize],
) {
	let instance = param.instances.access(descriptors).load(payload.instance_id as usize);
	let mesh = param.mesh.access(descriptors).load();
	let frame_data = param.frame_data.access(descriptors).load();
	let meshlet_id = payload.meshlet_offset + wg_id.x;
	let meshlet = mesh.meshlet(descriptors, meshlet_id as usize);
	let inv_id = inv_id.x as usize;

	let vertex_count = meshlet.vertices();
	let triangle_count = meshlet.triangles();
	unsafe {
		set_mesh_outputs_ext(vertex_count as u32, triangle_count as u32);
	}

	// process vertices
	// Safety: panics within pools mispile
	unsafe {
		for iter in 0..((vertex_count + MESH_WG_SIZE - 1) / MESH_WG_SIZE) {
			let i = iter * MESH_WG_SIZE + inv_id;
			let inbounds = i < vertex_count;
			let i = if inbounds { i } else { vertex_count - 1 };
			let position = transform_vertex(frame_data, instance, meshlet.load_vertex_unchecked(descriptors, i));
			if inbounds {
				*vtx_positions.index_unchecked_mut(i) = position;
				*vtx_meshlet_id.index_unchecked_mut(i) = meshlet_id;
			}
		}
	}

	// process primitives
	// Safety: panics within pools mispile
	unsafe {
		for iter in 0..((triangle_count + MESH_WG_SIZE - 1) / MESH_WG_SIZE) {
			let i = iter * MESH_WG_SIZE + inv_id;
			let inbounds = i < triangle_count;
			let i = if inbounds { i } else { triangle_count - 1 };
			let indices = meshlet.load_triangle_indices_unchecked(descriptors, i);
			if i < triangle_count {
				*prim_indices.index_unchecked_mut(i) = indices;
			}
		}
	}
}

fn transform_vertex(frame_data: FrameData, instance: MeshletInstance<Strong>, vertex: MeshletVertex) -> Vec4 {
	let camera = frame_data.camera;
	let worldspace = instance.transform().transform_point3(vertex.position());
	let cameraspace = camera.transform.transform_point3(worldspace.into());
	camera.perspective * Vec4::from((cameraspace, 1.))
}

#[bindless(fragment())]
pub fn meshlet_frag_meshlet_id(
	#[bindless(param_constants)] _param: &Params<'static>,
	#[spirv(flat)] vtx_meshlet_id: u32,
	frag_color: &mut Vec4,
) {
	pub const PHI: f32 = 1.618033988749894848204586834365638118_f32;
	let random = vtx_meshlet_id as f32 * PHI;

	*frag_color = Vec4::from((hsv2rgb_smooth(vec3(random, 1., 1.)), 1.));
}

/// Smooth HSV to RGB conversion
/// MIT by Inigo Quilez, from https://www.shadertoy.com/view/MsS3Wc
fn hsv2rgb_smooth(c: Vec3) -> Vec3 {
	fn modulo(x: Vec3, y: Vec3) -> Vec3 {
		x - y * Vec3::floor(x / y)
	}

	let rgb = Vec3::clamp(
		Vec3::abs(modulo(c.x * 6.0 + vec3(0.0, 4.0, 2.0), Vec3::splat(6.0)) - 3.0) - 1.0,
		Vec3::splat(0.0),
		Vec3::splat(1.0),
	);
	// cubic smoothing
	let rgb = rgb * rgb * (3.0 - 2.0 * rgb);
	c.z * Vec3::lerp(Vec3::splat(1.0), rgb, c.y)
}
