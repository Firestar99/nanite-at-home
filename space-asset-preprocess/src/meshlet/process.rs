use crate::gltf::Gltf;
use crate::image::encode::EncodeSettings;
use crate::image::image_processor::ImageProcessor;
use crate::material::pbr::{process_pbr_material, process_pbr_vertices};
use crate::meshlet::error::MeshletError;
use core::mem::size_of;
use glam::{Affine3A, Vec3};
use gltf::mesh::Mode;
use gltf::Primitive;
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset_disk::material::pbr::PbrMaterialDisk;
use space_asset_disk::meshlet::indices::triangle_indices_write_vec;
use space_asset_disk::meshlet::instance::MeshletInstanceDisk;
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::scene::MeshletSceneDisk;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use space_asset_disk::range::RangeU32;

#[profiling::function]
pub fn process_meshlets(gltf: &Gltf) -> anyhow::Result<MeshletSceneDisk> {
	let mut pbr_materials = None;
	let mut meshes_instances = None;
	rayon::in_place_scope(|scope| {
		scope.spawn(|_| pbr_materials = Some(process_materials(gltf)));
		scope.spawn(|_| meshes_instances = Some(process_meshes(gltf)));
	});
	let pbr_materials = pbr_materials.unwrap()?;
	let (meshes, instances) = meshes_instances.unwrap()?;

	Ok(MeshletSceneDisk {
		pbr_materials,
		meshes,
		instances,
	})
}

#[profiling::function]
fn process_materials(gltf: &Gltf) -> anyhow::Result<Vec<PbrMaterialDisk>> {
	let image_processor = ImageProcessor::new(gltf);
	let pbr_materials = {
		profiling::scope!("materials 1");
		gltf.materials()
			.map(|mat| process_pbr_material(gltf, &image_processor, mat))
			.collect::<Result<Vec<_>, _>>()?
	};

	let image_accessor = {
		profiling::scope!("images");
		let encode_settings = EncodeSettings::ultra_fast();
		image_processor.process(encode_settings)?
	};

	let pbr_materials = {
		profiling::scope!("materials 2");
		pbr_materials
			.into_iter()
			.map(|mat| mat.finish(&image_accessor))
			.collect::<Result<Vec<_>, _>>()?
	};

	Ok(pbr_materials)
}

#[profiling::function]
fn process_meshes(gltf: &Gltf) -> anyhow::Result<(Vec<MeshletMeshDisk>, Vec<MeshletInstanceDisk>)> {
	let mesh_primitives = {
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

	let (meshes, mesh2ids) = {
		let mut mesh2ids = Vec::with_capacity(mesh_primitives.len());
		let mut i = 0;
		let meshes = mesh_primitives
			.into_iter()
			.flat_map(|mesh| {
				let len = mesh.len() as u32;
				mesh2ids.push(RangeU32::new(i, i + len));
				i += len;
				mesh.into_iter()
			})
			.collect::<Vec<_>>();
		(meshes, mesh2ids)
	};

	let instances = {
		profiling::scope!("instance transformations");
		let scene = gltf.default_scene().ok_or(MeshletError::NoDefaultScene)?;
		let node_transforms = gltf.absolute_node_transformations(&scene, Affine3A::default());
		gltf.nodes()
			.flat_map(|node| {
				node.mesh().map(|mesh| MeshletInstanceDisk {
					transform: node_transforms[node.index()],
					mesh_ids: mesh2ids[mesh.index()],
				})
			})
			.collect::<Vec<_>>()
	};
	Ok((meshes, instances))
}

#[profiling::function]
fn process_mesh_primitive(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<MeshletMeshDisk> {
	if primitive.mode() != Mode::Triangles {
		Err(MeshletError::PrimitiveMustBeTriangleList)?;
	}

	let reader = primitive.reader(|b| gltf.buffer(b));
	let draw_vertices: Vec<_> = reader
		.read_positions()
		.ok_or(MeshletError::NoVertexPositions)?
		.map(Vec3::from)
		.collect();
	let draw_vertices_len = draw_vertices.len();

	let mut indices: Vec<_> = if let Some(indices) = reader.read_indices() {
		indices.into_u32().collect()
	} else {
		(0..draw_vertices_len as u32).collect()
	};

	{
		profiling::scope!("meshopt::optimize_vertex_cache");
		meshopt::optimize_vertex_cache_in_place(&mut indices, draw_vertices.len());
	}

	let out = {
		let adapter = VertexDataAdapter::new(bytemuck::cast_slice(&draw_vertices), size_of::<Vec3>(), 0).unwrap();
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
		.map(|i| DrawVertex {
			position: draw_vertices[i as usize],
			material_vertex_id: MaterialVertexId(i),
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
		pbr_material_vertices: process_pbr_vertices(gltf, primitive.clone(), draw_vertices_len)?,
		pbr_material_id: primitive.material().index().map(|i| i as u32),
	})
}
