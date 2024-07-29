use crate::material::light::{DirectionalLight, PointLight};
use crate::material::pbr::{PbrMaterialSample, SurfaceLocation};
use crate::material::radiance::Radiance;
use crate::renderer::frame_data::{DebugSettings, FrameData};
use crate::utils::gpurng::GpuRng;
use crate::utils::hsv::hsv2rgb_smooth;
use glam::{vec3, UVec3, Vec2, Vec3, Vec4, Vec4Swizzles};
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::MeshletMesh;
use space_asset::meshlet::mesh2instance::MeshletMesh2Instance;
use space_asset::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use spirv_std::arch::{
	atomic_i_add, emit_mesh_tasks_ext_payload, set_mesh_outputs_ext, subgroup_non_uniform_ballot,
	subgroup_non_uniform_ballot_bit_count, subgroup_non_uniform_elect, workgroup_memory_barrier_with_group_sync,
	GroupOperation, IndexUnchecked,
};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::Sampler;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::{bindless, BufferContent};
use vulkano_bindless_shaders::descriptor::reference::{Strong, Transient};
use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[derive(Copy, Clone, BufferContent)]
pub struct Params<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub mesh2instance: MeshletMesh2Instance<Transient<'a>, Strong>,
	pub sampler: TransientDesc<'a, Sampler>,
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

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct InterpolationVertex {
	world_pos: Vec3,
	normals: Vec3,
	tex_coords: Vec2,
	meshlet_debug_hue: f32,
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
			let position = frame_data
				.camera
				.transform_vertex(instance.transform, draw_vertex.position);
			let pbr_vertex = meshlet.load_pbr_material_vertex_unchecked(descriptors, draw_vertex.material_vertex_id);
			let normals = frame_data
				.camera
				.transform_normal(instance.transform, pbr_vertex.normals);
			let vertex = InterpolationVertex {
				world_pos: position.world_space,
				normals: normals.world_space,
				tex_coords: pbr_vertex.tex_coords,
				meshlet_debug_hue: GpuRng(meshlet_id).next_f32(),
			};

			if inbounds {
				*out_positions.index_unchecked_mut(i) = position.clip_space;
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

#[bindless(fragment())]
pub fn meshlet_frag_meshlet_id(
	#[bindless(descriptors)] descriptors: &Descriptors,
	#[bindless(param_constants)] param: &Params<'static>,
	out_vertex: InterpolationVertex,
	frag_color: &mut Vec4,
) {
	let frame_data = param.frame_data.access(descriptors).load();
	let debug_settings = frame_data.debug_settings();
	*frag_color = match debug_settings {
		DebugSettings::None => material_eval(descriptors, param, frame_data, out_vertex),
		DebugSettings::BaseColor => base_color(descriptors, param, out_vertex),
		DebugSettings::MeshletIdOverlay => {
			let base_color = material_eval(descriptors, param, frame_data, out_vertex);
			Vec4::from((
				Vec3::lerp(base_color.xyz(), meshlet_debug_color(out_vertex), 0.1),
				base_color.w,
			))
		}
		DebugSettings::MeshletId => Vec4::from((meshlet_debug_color(out_vertex), 1.)),
		DebugSettings::VertexNormals => Vec4::from((out_vertex.normals, 1.)),
		DebugSettings::VertexTexCoords => Vec4::from((out_vertex.tex_coords, 0., 1.)),
	};
	if frag_color.w < 0.01 {
		spirv_std::arch::kill();
	}
}

fn material_eval(
	descriptors: &Descriptors,
	param: &Params<'static>,
	frame_data: FrameData,
	out_vertex: InterpolationVertex,
) -> Vec4 {
	// let point_lights = [PointLight {
	// 	position: Vec3::new(1.8, -2., -2.7),
	// 	color: Vec3::new(1., 1., 1.) * 20.,
	// }];
	// let directional_lights = [DirectionalLight {
	// 	direction: Vec3::new(0., 1., 0.).normalize(),
	// 	color: Vec3::new(0., 0., 0.),
	// }];

	let point_lights = [PointLight {
		position: Vec3::new(0., -0., -0.),
		color: Radiance(Vec3::new(1., 1., 1.)) * 0.,
	}];
	let directional_lights = [DirectionalLight {
		direction: Vec3::new(-0.3, 1., -0.1).normalize(),
		color: Radiance(Vec3::new(1., 1., 1.)) * 20.,
	}];
	let ambient_light = Radiance(Vec3::splat(0.02));

	let surface_location = SurfaceLocation::new(
		out_vertex.world_pos,
		frame_data.camera.transform.translation(),
		out_vertex.normals,
		out_vertex.tex_coords,
	);
	let mesh = param.mesh2instance.mesh.access(descriptors).load();
	let sampler = param.sampler.access(descriptors);
	let sampled = mesh.pbr_material.sample(descriptors, *sampler, surface_location);

	let mut lo = Radiance(Vec3::ZERO);
	for i in 0..point_lights.len() {
		lo += sampled.evaluate_point_light(point_lights[i]);
	}
	for i in 0..directional_lights.len() {
		lo += sampled.evaluate_directional_light(directional_lights[i]);
	}
	lo += sampled.ambient_light(ambient_light);
	Vec4::from((lo.tone_map_reinhard(), sampled.alpha))
}

fn base_color(descriptors: &Descriptors, param: &Params<'static>, out_vertex: InterpolationVertex) -> Vec4 {
	let mesh = param.mesh2instance.mesh.access(descriptors).load();
	let sampler = param.sampler.access(descriptors);
	let base_color: Vec4 = mesh
		.pbr_material
		.base_color
		.access(descriptors)
		.sample(*sampler, out_vertex.tex_coords)
		* Vec4::from(mesh.pbr_material.base_color_factor);
	base_color
}

fn meshlet_debug_color(out_vertex: InterpolationVertex) -> Vec3 {
	hsv2rgb_smooth(vec3(out_vertex.meshlet_debug_hue, 1., 1.))
}
