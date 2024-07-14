use crate::gltf::Gltf;
use crate::image::encode::EncodeSettings;
use crate::material::pbr::{process_pbr_material, process_pbr_vertices};
use crate::meshlet::error::MeshletError;
use glam::{Affine3A, Mat3, Vec3};
use gltf::mesh::Mode;
use gltf::Primitive;
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset::meshlet::indices::triangle_indices_write_vec;
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
use space_asset::meshlet::offset::MeshletOffset;
use space_asset::meshlet::scene::MeshletSceneDisk;
use space_asset::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use std::mem;

pub fn process_meshlets(gltf: &Gltf) -> anyhow::Result<MeshletSceneDisk> {
	profiling::scope!("process");

	let texture_encode_settings = EncodeSettings::ultra_fast();
	let pbr_materials = {
		profiling::scope!("process materials");
		gltf.materials()
			.collect::<Vec<_>>()
			.into_par_iter()
			.map(|mat| process_pbr_material(gltf, mat, texture_encode_settings))
			.collect::<Result<Vec<_>, _>>()?
	};

	let meshes_primitives = {
		profiling::scope!("process meshes");
		gltf.meshes()
			.collect::<Vec<_>>()
			.into_par_iter()
			.map(|mesh| {
				let vec = mesh.primitives().collect::<SmallVec<[_; 4]>>();
				vec.into_par_iter()
					.map(|primitive| process_mesh_primitive(gltf, primitive.clone()))
					.collect::<Result<Vec<_>, _>>()
			})
			.collect::<Result<Vec<_>, _>>()?
	};

	let mesh2instance = {
		profiling::scope!("instance transformations");
		let scene = gltf.default_scene().ok_or(MeshletError::NoDefaultScene)?;
		let node_transforms = gltf.absolute_node_transformations(
			&scene,
			Affine3A::from_mat3(Mat3 {
				y_axis: Vec3::new(0., -1., 0.),
				..Mat3::default()
			}),
		);
		let mut mesh2instance = (0..gltf.meshes().len()).map(|_| Vec::new()).collect::<Vec<_>>();
		for node in gltf.nodes() {
			if let Some(mesh) = node.mesh() {
				mesh2instance[mesh.index()].push(MeshletInstance::new(node_transforms[node.index()]));
			}
		}
		mesh2instance
	};

	let mesh2instances = meshes_primitives
		.into_iter()
		.zip(mesh2instance.into_iter())
		.flat_map(|(mesh_primitives, instances)| {
			mesh_primitives
				.into_iter()
				.map(move |primitive| MeshletMesh2InstanceDisk {
					mesh: primitive,
					instances: instances.clone(),
				})
		})
		.collect();

	Ok(MeshletSceneDisk {
		pbr_materials,
		mesh2instances,
	})
}

fn process_mesh_primitive(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<MeshletMeshDisk> {
	profiling::scope!("process_mesh_primitive");
	if primitive.mode() != Mode::Triangles {
		Err(MeshletError::PrimitiveMustBeTriangleList)?;
	}

	let reader = primitive.reader(|b| gltf.buffer(b));
	let vertex_positions: Vec<_> = reader
		.read_positions()
		.ok_or(MeshletError::NoVertexPositions)?
		.map(|pos| Vec3::from(pos))
		.collect();

	let mut indices: Vec<_> = if let Some(indices) = reader.read_indices() {
		indices.into_u32().collect()
	} else {
		(0..vertex_positions.len() as u32).collect()
	};

	{
		profiling::scope!("meshopt::optimize_vertex_cache");
		meshopt::optimize_vertex_cache_in_place(&mut indices, vertex_positions.len());
	}

	let out = {
		let adapter =
			VertexDataAdapter::new(bytemuck::cast_slice(&*vertex_positions), mem::size_of::<Vec3>(), 0).unwrap();
		let mut out = {
			profiling::scope!("meshopt::build_meshlets");
			meshopt::build_meshlets(
				&indices,
				&adapter,
				MESHLET_MAX_VERTICES as usize,
				MESHLET_MAX_TRIANGLES as usize,
				0.,
			)
		};
		// resize vertex buffer appropriately
		out.vertices.truncate(
			out.meshlets
				.last()
				.map(|m| m.vertex_offset as usize + m.vertex_count as usize)
				.unwrap_or(0),
		);
		out
	};

	let indices = out.iter().flat_map(|m| m.triangles).copied().collect::<Vec<_>>();
	let triangles = triangle_indices_write_vec(indices.iter().copied().map(u32::from));

	let draw_vertices = out
		.vertices
		.into_iter()
		.map(|i| {
			DrawVertex {
				position: vertex_positions[i as usize],
				material_vertex_id: MaterialVertexId(i),
			}
			.encode()
		})
		.collect();

	let mut triangle_start = 0;
	let meshlets = out
		.meshlets
		.into_iter()
		.map(|m| {
			let data = MeshletData {
				draw_vertex_offset: MeshletOffset::new(m.vertex_offset as usize, m.vertex_count as usize),
				triangle_offset: MeshletOffset::new(triangle_start, m.triangle_count as usize),
			};
			triangle_start += m.triangle_count as usize;
			data
		})
		.collect();

	Ok(MeshletMeshDisk {
		draw_vertices,
		meshlets,
		triangles,
		pbr_material_vertices: process_pbr_vertices(gltf, primitive.clone())?,
		pbr_material_id: primitive.material().index().unwrap() as u32,
	})
}
