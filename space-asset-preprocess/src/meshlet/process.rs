use crate::gltf::Gltf;
use crate::image::image_processor::ImageProcessor;
use crate::material::pbr::{process_pbr_material, process_pbr_vertices};
use crate::meshlet::error::MeshletError;
use crate::meshlet::lod_mesh::LodMesh;
use crate::meshlet::lod_tree_gen::border_tracker::process_lod_tree;
use crate::meshlet::mesh::MeshletMesh;
use glam::{Affine3A, Vec3};
use gltf::Primitive;
use gltf::mesh::Mode;
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset_disk::image::{EncodeSettings, ImageStorage};
use space_asset_disk::material::pbr::PbrMaterialDisk;
use space_asset_disk::meshlet::indices::triangle_indices_write_vec;
use space_asset_disk::meshlet::instance::MeshletInstanceDisk;
use space_asset_disk::meshlet::lod_level_bitmask::LodLevelBitmask;
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::scene::MeshletSceneDisk;
use space_asset_disk::meshlet::stats::{MeshletSceneStats, SourceMeshStats};
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use space_asset_disk::range::RangeU32;
use space_asset_disk::shape::sphere::Sphere;
use std::mem::{offset_of, size_of};
use std::ops::Range;

pub fn process_meshlets(gltf: &Gltf) -> anyhow::Result<MeshletSceneDisk> {
	profiling::function_scope!();
	let mut pbr_materials = None;
	let mut meshes_instances = None;
	rayon::in_place_scope(|scope| {
		scope.spawn(|_| pbr_materials = Some(process_materials(gltf)));
		scope.spawn(|_| meshes_instances = Some(process_meshes(gltf)));
	});
	let (image_storage, pbr_materials) = pbr_materials.unwrap()?;
	let (meshes, instances, src_stats) = meshes_instances.unwrap()?;

	Ok(MeshletSceneDisk {
		image_storage,
		pbr_materials,
		meshes,
		instances,
		stats: MeshletSceneStats { source: src_stats },
	})
}

fn process_materials(gltf: &Gltf) -> anyhow::Result<(ImageStorage, Vec<PbrMaterialDisk>)> {
	profiling::function_scope!();
	let image_processor = ImageProcessor::new(gltf);
	let pbr_materials = {
		profiling::scope!("materials");
		gltf.materials()
			.map(|mat| process_pbr_material(gltf, &image_processor, mat))
			.collect::<Result<Vec<_>, _>>()?
	};

	let image_storage = {
		profiling::scope!("images");
		let encode_settings = EncodeSettings::ultra_fast();
		image_processor.process(encode_settings)?
	};

	Ok((image_storage, pbr_materials))
}

fn process_meshes(gltf: &Gltf) -> anyhow::Result<(Vec<MeshletMeshDisk>, Vec<MeshletInstanceDisk>, SourceMeshStats)> {
	profiling::function_scope!();
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
		profiling::scope!("mesh2ids");
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
					world_from_local: node_transforms[node.index()],
					mesh_ids: mesh2ids[mesh.index()],
				})
			})
			.collect::<Vec<_>>()
	};

	let stats = {
		profiling::scope!("scene stats");
		instances
			.iter()
			.flat_map(|instance| {
				Range::<u32>::from(instance.mesh_ids)
					.map(|mesh_id| meshes[mesh_id as usize].stats.transform(instance.world_from_local))
			})
			.sum()
	};

	Ok((meshes, instances, stats))
}

fn process_mesh_primitive(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<MeshletMesh> {
	profiling::function_scope!();
	if primitive.mode() != Mode::Triangles {
		Err(MeshletError::PrimitiveMustBeTriangleList)?;
	}

	let reader = primitive.reader(|b| gltf.buffer(b));
	let mut src_vertices: Vec<_> = reader
		.read_positions()
		.ok_or(MeshletError::NoVertexPositions)?
		.enumerate()
		.map(|(i, pos)| DrawVertex {
			position: Vec3::from_array(pos),
			material_vertex_id: MaterialVertexId(i as u32),
		})
		.collect();
	let src_vertices_len = src_vertices.len();

	let mut indices: Vec<_> = if let Some(indices) = reader.read_indices() {
		indices.into_u32().collect()
	} else {
		(0..src_vertices_len as u32).collect()
	};

	let lod_mesh = lod_mesh_build_meshlets(&mut indices, &mut src_vertices, None, 0.);

	let stats = SourceMeshStats {
		unique_vertices: src_vertices_len as u32,
		triangles: (indices.len() / 3) as u32,
		meshlets: lod_mesh.meshlets.len() as u32,
		meshlet_vertices: lod_mesh.draw_vertices.len() as u32,
		bounds_min: src_vertices
			.iter()
			.map(|v| v.position)
			.reduce(Vec3::min)
			.unwrap_or(Vec3::ZERO),
		bounds_max: src_vertices
			.iter()
			.map(|v| v.position)
			.reduce(Vec3::max)
			.unwrap_or(Vec3::ZERO),
	};

	let pbr_material_vertices = process_pbr_vertices(gltf, primitive.clone())?;
	assert_eq!(pbr_material_vertices.len(), src_vertices_len);

	Ok(MeshletMesh {
		lod_mesh,
		pbr_material_vertices,
		pbr_material_id: primitive.material().index().map(|i| i as u32),
		stats,
	})
}

pub fn lod_mesh_build_meshlets(
	indices: &mut [u32],
	draw_vertices: &mut Vec<DrawVertex>,
	bounds: Option<Sphere>,
	error: f32,
) -> LodMesh {
	profiling::function_scope!();
	{
		profiling::scope!("meshopt::optimize_vertex_fetch_in_place");
		let vertex_cnt = meshopt::optimize_vertex_fetch_in_place(indices, draw_vertices);
		draw_vertices.truncate(vertex_cnt);
	}

	{
		profiling::scope!("meshopt::optimize_vertex_cache_in_place");
		meshopt::optimize_vertex_cache_in_place(indices, draw_vertices.len());
	}

	let adapter = VertexDataAdapter::new(
		bytemuck::cast_slice::<DrawVertex, u8>(draw_vertices),
		size_of::<DrawVertex>(),
		offset_of!(DrawVertex, position),
	)
	.unwrap();

	let out = {
		profiling::scope!("meshopt::build_meshlets");
		meshopt::build_meshlets(
			indices,
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
