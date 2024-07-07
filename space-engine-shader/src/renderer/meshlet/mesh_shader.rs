use crate::renderer::frame_data::FrameData;
use glam::{UVec3, Vec4};
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::MeshletMesh;
use space_asset::meshlet::mesh2instance::MeshletMesh2Instance;
use space_asset::meshlet::vertex::{DrawVertex, MaterialVertex};
use space_asset::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use spirv_std::arch::{
	atomic_i_add, emit_mesh_tasks_ext_payload, set_mesh_outputs_ext, subgroup_non_uniform_ballot,
	subgroup_non_uniform_ballot_bit_count, subgroup_non_uniform_elect, workgroup_memory_barrier_with_group_sync,
	GroupOperation, IndexUnchecked,
};
use spirv_std::memory::{Scope, Semantics};
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::{bindless, BufferContent};
use vulkano_bindless_shaders::descriptor::reference::{Strong, Transient};
use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[derive(Copy, Clone, BufferContent)]
pub struct Params<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub mesh2instance: MeshletMesh2Instance<Transient<'a>, Strong>,
}

#[derive(Copy, Clone, BufferContent)]
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

		let mesh = param.mesh2instance.mesh.access(descriptors).load();
		let instances = param.mesh2instance.instances.access(descriptors);
		let instance = instances.load_unchecked(wg_instance_id as usize);

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
	_instance: MeshletInstance,
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

struct InterpolationVertex {
	material: MaterialVertex,
	// meshlet_id: u32,
}

pub const MESH_WG_SIZE: usize = 32;

const_assert_eq!(MESH_WG_SIZE, 32);
const_assert_eq!(MESHLET_MAX_VERTICES, 64);
const_assert_eq!(MESHLET_MAX_TRIANGLES, 124);
#[bindless(mesh_ext(threads(32), output_vertices = 64, output_primitives_ext = 124, output_triangles_ext))]
pub fn meshlet_mesh(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
	#[spirv(task_payload_workgroup_ext)] payload: &Payload,
	#[spirv(primitive_triangle_indices_ext)] prim_indices: &mut [UVec3; MESHLET_MAX_TRIANGLES as usize],
	#[spirv(position)] out_positions: &mut [Vec4; MESHLET_MAX_VERTICES as usize],
	out_vertex: &mut [InterpolationVertex; MESHLET_MAX_TRIANGLES as usize],
) {
	let instances = param.mesh2instance.instances.access(descriptors);
	let instance = instances.load(payload.instance_id as usize);
	let mesh = param.mesh2instance.mesh.access(descriptors).load();
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

			let draw_vertex = meshlet.load_draw_vertex_unchecked(descriptors, i);
			let position = transform_vertex(frame_data, instance, draw_vertex);
			let vertex = InterpolationVertex {
				material: meshlet.load_material_vertex_unchecked(descriptors, draw_vertex.material_vertex_id),
				// meshlet_id,
			};

			if inbounds {
				*out_positions.index_unchecked_mut(i) = position;
				*out_vertex.index_unchecked_mut(i) = vertex;
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

			let indices = meshlet.load_triangle_unchecked(descriptors, i);

			if i < triangle_count {
				*prim_indices.index_unchecked_mut(i) = indices;
			}
		}
	}
}

fn transform_vertex(frame_data: FrameData, instance: MeshletInstance, vertex: DrawVertex) -> Vec4 {
	let camera = frame_data.camera;
	let worldspace = instance.transform().transform_point3(vertex.position);
	let cameraspace = camera.transform.transform_point3(worldspace.into());
	camera.perspective * Vec4::from((cameraspace, 1.))
}

#[bindless(fragment())]
pub fn meshlet_frag_meshlet_id(
	#[bindless(param_constants)] _param: &Params<'static>,
	out_vertex: InterpolationVertex,
	frag_color: &mut Vec4,
) {
	// *frag_color = Vec4::from((GpuRng(out_vertex.meshlet_id).next_hue(), 1.));
	*frag_color = Vec4::from((out_vertex.material.normals, 1.));
	// *frag_color = Vec4::from((out_vertex.material.tex_coords, 0., 1.));
}
