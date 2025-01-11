use crate::gltf::Gltf;
use crate::image::encode::EncodeSettings;
use crate::image::image_processor::ImageProcessor;
use crate::material::pbr::{process_pbr_material, process_pbr_vertices};
use crate::meshlet::error::MeshletError;
use crate::meshlet::lod_mesh::LodMesh;
use crate::meshlet::lod_tree_gen::border_tracker::BorderTracker;
use crate::meshlet::mesh::MeshletMesh;
use glam::{Affine3A, Vec3};
use gltf::mesh::Mode;
use gltf::Primitive;
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset_disk::material::pbr::PbrMaterialDisk;
use space_asset_disk::meshlet::indices::triangle_indices_write_vec;
use space_asset_disk::meshlet::instance::MeshletInstanceDisk;
use space_asset_disk::meshlet::lod_level_bitmask::LodLevelBitmask;
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::scene::MeshletSceneDisk;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use space_asset_disk::range::RangeU32;
use space_asset_disk::shape::sphere::Sphere;
use std::mem::{offset_of, size_of};

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
					.map(|primitive| {
						let mesh = process_mesh_primitive(gltf, primitive.clone())?;
						let mesh = process_lod_tree(mesh)?.to_meshlet_mesh_disk()?;
						Ok::<_, anyhow::Error>(mesh)
					})
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
fn process_mesh_primitive(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<MeshletMesh> {
	if primitive.mode() != Mode::Triangles {
		Err(MeshletError::PrimitiveMustBeTriangleList)?;
	}

	let reader = primitive.reader(|b| gltf.buffer(b));
	let draw_vertices: Vec<_> = reader
		.read_positions()
		.ok_or(MeshletError::NoVertexPositions)?
		.enumerate()
		.map(|(i, pos)| DrawVertex {
			position: Vec3::from_array(pos),
			material_vertex_id: MaterialVertexId(i as u32),
		})
		.collect();
	let draw_vertices_len = draw_vertices.len();

	let indices: Vec<_> = if let Some(indices) = reader.read_indices() {
		indices.into_u32().collect()
	} else {
		(0..draw_vertices_len as u32).collect()
	};

	let lod_mesh = lod_mesh_build_meshlets(indices, draw_vertices, None, 0.);

	Ok(MeshletMesh {
		lod_mesh,
		pbr_material_vertices: process_pbr_vertices(gltf, primitive.clone(), draw_vertices_len)?,
		pbr_material_id: primitive.material().index().map(|i| i as u32),
	})
}

#[profiling::function]
pub fn lod_mesh_build_meshlets(
	mut indices: Vec<u32>,
	mut draw_vertices: Vec<DrawVertex>,
	bounds: Option<Sphere>,
	error: f32,
) -> LodMesh {
	{
		profiling::scope!("meshopt::optimize_vertex_fetch_in_place");
		let vertex_cnt = meshopt::optimize_vertex_fetch_in_place(&mut indices, &mut draw_vertices);
		draw_vertices.truncate(vertex_cnt);
	}

	{
		profiling::scope!("meshopt::optimize_vertex_cache_in_place");
		meshopt::optimize_vertex_cache_in_place(&mut indices, draw_vertices.len());
	}

	let adapter = VertexDataAdapter::new(
		bytemuck::cast_slice::<DrawVertex, u8>(&draw_vertices),
		size_of::<DrawVertex>(),
		offset_of!(DrawVertex, position),
	)
	.unwrap();

	let out = {
		profiling::scope!("meshopt::build_meshlets");
		meshopt::build_meshlets(
			&indices,
			&adapter,
			MESHLET_MAX_VERTICES as usize,
			MESHLET_MAX_TRIANGLES as usize,
			0.,
		)
	};

	{
		profiling::scope!("lod_mesh_from_meshopt");
		let indices = out.iter().flat_map(|m| m.triangles).copied().collect::<Vec<_>>();
		let triangles = triangle_indices_write_vec(indices.into_iter().map(u32::from));

		// resize vertex buffer appropriately
		let last_vertex = out
			.meshlets
			.last()
			.map(|m| m.vertex_offset as usize + m.vertex_count as usize)
			.unwrap_or(0);
		let draw_vertices = out
			.vertices
			.iter()
			.take(last_vertex)
			.map(|i| draw_vertices[*i as usize])
			.collect();

		let mut triangle_start = 0;
		let meshlets = out
			.meshlets
			.iter()
			.zip(out.iter())
			.map(|(m, meshlet)| {
				let data = MeshletData {
					draw_vertex_offset: MeshletOffset::new(m.vertex_offset as usize, m.vertex_count as usize),
					triangle_offset: MeshletOffset::new(triangle_start, m.triangle_count as usize),
					bounds: bounds.unwrap_or_else(|| {
						let bounds = meshopt::compute_meshlet_bounds(meshlet, &adapter);
						Sphere::new(Vec3::from_array(bounds.center), bounds.radius)
					}),
					parent_bounds: Sphere::default(),
					error,
					parent_error: f32::INFINITY,
					lod_level_bitmask: LodLevelBitmask::default(),
					_pad: [0; 1],
				};
				triangle_start += m.triangle_count as usize;
				data
			})
			.collect::<Vec<_>>();

		LodMesh {
			draw_vertices,
			meshlets,
			triangles,
		}
	}
}

fn process_lod_tree(mut mesh: MeshletMesh) -> anyhow::Result<MeshletMesh> {
	let max_lod_level = 15;

	let mut prev_lod = mesh.lod_mesh;
	mesh.lod_mesh = LodMesh::default();
	for m in &mut prev_lod.meshlets {
		m.lod_level_bitmask = LodLevelBitmask(1);
	}

	let mut lod_levels = 1..max_lod_level;
	for lod_level in &mut lod_levels {
		let lod_faction = lod_level as f32 / max_lod_level as f32;
		let mut lod =
			BorderTracker::from_meshlet_mesh(&mut prev_lod).simplify(lod_faction, &mesh.pbr_material_vertices);
		for m in &mut lod.meshlets {
			m.lod_level_bitmask = LodLevelBitmask(1 << lod_level);
		}

		mesh.lod_mesh.append(&mut prev_lod);
		prev_lod = lod;

		if prev_lod.meshlets.len() <= 1 {
			break;
		}
	}

	for m in &mut prev_lod.meshlets {
		m.lod_level_bitmask |= LodLevelBitmask(!0 << lod_levels.start);
	}
	mesh.lod_mesh.append(&mut prev_lod);
	Ok(mesh)
}
