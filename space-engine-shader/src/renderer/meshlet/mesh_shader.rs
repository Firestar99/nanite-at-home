use crate::material::pbr::{PbrMaterialSample, SurfaceLocation};
use crate::renderer::compacting_alloc_buffer::CompactingAllocBufferReader;
use crate::renderer::frame_data::{DebugSettings, FrameData};
use crate::renderer::meshlet::intermediate::MeshletInstance;
use crate::utils::gpurng::GpuRng;
use glam::{UVec3, Vec2, Vec3, Vec4};
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, Strong, TransientDesc};
use space_asset_shader::meshlet::mesh::MeshletMesh;
use space_asset_shader::meshlet::scene::MeshletScene;
use space_asset_shader::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use spirv_std::arch::{set_mesh_outputs_ext, IndexUnchecked};
use spirv_std::Sampler;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub scene: TransientDesc<'a, Buffer<MeshletScene<Strong>>>,
	pub sampler: TransientDesc<'a, Sampler>,
	pub compacting_alloc_buffer: CompactingAllocBufferReader<'a, MeshletInstance>,
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
#[repr(C)]
struct InterpolationVertex {
	tangent: Vec4,
	world_pos: Vec3,
	normal: Vec3,
	tex_coord: Vec2,
}

pub const MESH_WG_SIZE: usize = 32;

const_assert_eq!(MESH_WG_SIZE, 32);
const_assert_eq!(MESHLET_MAX_VERTICES, 64);
const_assert_eq!(MESHLET_MAX_TRIANGLES, 124);
#[bindless(mesh_ext(threads(32), output_vertices = 64, output_primitives_ext = 124, output_triangles_ext))]
pub fn meshlet_mesh(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
	#[spirv(primitive_triangle_indices_ext)] prim_indices: &mut [UVec3; MESHLET_MAX_TRIANGLES as usize],
	#[spirv(per_primitive_ext)] out_debug_hue: &mut [f32; MESHLET_MAX_TRIANGLES as usize],
	#[spirv(position)] out_positions: &mut [Vec4; MESHLET_MAX_VERTICES as usize],
	out_mesh_id: &mut [u32; MESHLET_MAX_VERTICES as usize],
	out_vertex: &mut [InterpolationVertex; MESHLET_MAX_VERTICES as usize],
) {
	let meshlet_instance_id = wg_id.x;
	let inv_id = inv_id.x as usize;

	let frame_data = param.frame_data.access(&descriptors).load();
	let scene = param.scene.access(&descriptors).load();
	let meshlet_instance = param
		.compacting_alloc_buffer
		.access(&descriptors)
		.read(meshlet_instance_id);
	let instance = scene
		.instances
		.access(&descriptors)
		.load(meshlet_instance.instance_id as usize);
	let mesh: MeshletMesh<Strong> = scene
		.meshes
		.access(&descriptors)
		.load(meshlet_instance.mesh_id as usize);
	let meshlet = mesh.meshlet(&descriptors, meshlet_instance.meshlet_id as usize);

	let vertex_count = meshlet.vertices();
	let triangle_count = meshlet.triangles();
	unsafe {
		set_mesh_outputs_ext(vertex_count as u32, triangle_count as u32);
	}

	// process vertices
	// Safety: panics within loops mispile
	unsafe {
		for iter in 0..((vertex_count + MESH_WG_SIZE - 1) / MESH_WG_SIZE) {
			let i = iter * MESH_WG_SIZE + inv_id;
			let inbounds = i < vertex_count;
			let i = if inbounds { i } else { vertex_count - 1 };

			let draw_vertex = meshlet.load_draw_vertex(&descriptors, i);
			let position = frame_data
				.camera
				.transform_vertex(instance.transform, draw_vertex.position);
			let pbr_vertex = meshlet.load_pbr_material_vertex(&descriptors, draw_vertex.material_vertex_id);
			let vertex = InterpolationVertex {
				world_pos: position.world_space,
				normal: pbr_vertex.normal,
				tangent: pbr_vertex.tangent,
				tex_coord: pbr_vertex.tex_coord,
			};

			if inbounds {
				*out_positions.index_unchecked_mut(i) = position.clip_space;
				*out_mesh_id.index_unchecked_mut(i) = meshlet_instance.mesh_id;
				*out_vertex.index_unchecked_mut(i) = vertex;
			}
		}
	}

	let debug_hue = |primitive_id: u32| {
		let seed = match frame_data.debug_settings() {
			DebugSettings::MeshletIdOverlay | DebugSettings::MeshletId => meshlet_instance.meshlet_id,
			DebugSettings::TriangleIdOverlay | DebugSettings::TriangleId => {
				meshlet_instance.meshlet_id.wrapping_add(primitive_id)
			}
			DebugSettings::LodLevel => 32 - leading_zeros(meshlet.lod_level_bitmask.0),
			_ => return 0.,
		};
		GpuRng(seed.wrapping_add(1)).next_f32()
	};

	// process primitives
	// Safety: panics within pools mispile
	unsafe {
		for iter in 0..((triangle_count + MESH_WG_SIZE - 1) / MESH_WG_SIZE) {
			let i = iter * MESH_WG_SIZE + inv_id;
			let inbounds = i < triangle_count;
			let i = if inbounds { i } else { triangle_count - 1 };

			let indices = meshlet.load_triangle(&descriptors, i);
			let debug_hue = debug_hue(i as u32);

			if i < triangle_count {
				*prim_indices.index_unchecked_mut(i) = indices;
				*out_debug_hue.index_unchecked_mut(i) = debug_hue;
			}
		}
	}
}

pub fn leading_zeros(mut x: u32) -> u32 {
	// Keep shifting x by one until leftmost bit
	// does not become 1.
	let total_bits = core::mem::size_of_val(&x) * 8;

	let mut res = 0;
	while (x & (1 << (total_bits - 1))) == 0 {
		x = x << 1;
		res += 1;
	}
	res
}

#[bindless(fragment())]
pub fn meshlet_fragment_g_buffer(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(per_primitive_ext)] out_debug_hue: f32,
	#[spirv(flat)] out_mesh_id: u32,
	out_vertex: InterpolationVertex,
	frag_albedo: &mut Vec4,
	frag_normal: &mut Vec4,
	frag_roughness_metallic: &mut Vec4,
) {
	let scene = param.scene.access(&descriptors).load();
	let mesh: MeshletMesh<Strong> = scene.meshes.access(&descriptors).load(out_mesh_id as usize);
	let frame_data = param.frame_data.access(&descriptors).load();
	let loc = SurfaceLocation::new(
		out_vertex.world_pos,
		frame_data.camera.transform.translation(),
		out_vertex.normal,
		out_vertex.tangent,
		out_vertex.tex_coord,
	);
	let mut sampled = mesh
		.pbr_material
		.sample(&descriptors, param.sampler.access(&descriptors), loc);
	match frame_data.debug_settings() {
		DebugSettings::VertexNormals => sampled.normal = loc.vertex_normal.normalize(),
		_ => (),
	}

	if sampled.alpha < 0.01 {
		spirv_std::arch::kill();
	}

	*frag_albedo = Vec4::from((sampled.albedo, sampled.alpha));
	*frag_normal = Vec4::from((sampled.normal * 0.5 + 0.5, out_debug_hue));
	*frag_roughness_metallic = Vec4::from((sampled.roughness, sampled.metallic, 1., 1.));
}
