use space_asset_disk::material::pbr::PbrVertex;
use space_asset_disk::meshlet::indices::{CompressedIndices, INDICES_PER_WORD};
use space_asset_disk::meshlet::instance::MeshletInstanceDisk;
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::scene::MeshletSceneDisk;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use std::collections::HashMap;

pub enum MergeStrategy {
	MergeSingleInstance,
}

pub fn merge_meshlets(scene: MeshletSceneDisk, _strategy: MergeStrategy) -> anyhow::Result<MeshletSceneDisk> {
	profiling::scope!("merge_meshlets");
	let MeshletSceneDisk {
		pbr_materials,
		mesh2instances,
	} = scene;

	let mut many_instances = Vec::<MeshletMesh2InstanceDisk>::new();
	let mut single_instances = Vec::<MeshletMesh2InstanceDisk>::new();
	for m2i in mesh2instances {
		if m2i.instances.len() == 1 {
			single_instances.push(m2i)
		} else {
			many_instances.push(m2i)
		}
	}

	let mut mat2mesh = HashMap::<Option<u32>, Vec<MeshletMesh2InstanceDisk>>::new();
	for m2i in single_instances {
		mat2mesh.entry(m2i.mesh.pbr_material_id).or_default().push(m2i);
	}

	let mut mesh2instances = many_instances;
	for (mat, meshes) in mat2mesh {
		let merged_mesh = merge(
			mat,
			meshes.iter().map(|m2i| (&m2i.mesh, *m2i.instances.first().unwrap())),
		);
		mesh2instances.push(MeshletMesh2InstanceDisk {
			mesh: merged_mesh,
			instances: Vec::from([MeshletInstanceDisk::default()]),
		});
	}

	Ok(MeshletSceneDisk {
		pbr_materials,
		mesh2instances,
	})
}

fn merge<'a>(
	pbr_material_id: Option<u32>,
	meshlets: impl Iterator<Item = (&'a MeshletMeshDisk, MeshletInstanceDisk)>,
) -> MeshletMeshDisk {
	let mut out = MeshletMeshDisk {
		meshlets: Vec::new(),
		draw_vertices: Vec::new(),
		triangles: Vec::new(),
		pbr_material_id,
		pbr_material_vertices: Vec::new(),
	};

	for (mesh, instance) in meshlets {
		assert_eq!(mesh.pbr_material_id, pbr_material_id);

		let pbr_start = out.pbr_material_vertices.len() as u32;
		let normal_transform = instance.transform.matrix3.inverse().transpose();
		out.pbr_material_vertices
			.extend(mesh.pbr_material_vertices.iter().map(|v| PbrVertex {
				normals: normal_transform * v.normals,
				..*v
			}));

		let draw_start = out.draw_vertices.len();
		out.draw_vertices.extend(mesh.draw_vertices.iter().map(|v| DrawVertex {
			material_vertex_id: MaterialVertexId(v.material_vertex_id.0 + pbr_start),
			position: instance.transform.transform_point3(v.position),
		}));

		// must always stay aligned to a multiple of triangles = 3 indices
		// as we don't want to rewrite CompressedIndices, we use a bit more padding
		let triangle_start = {
			let indices_start = out.triangles.len() * INDICES_PER_WORD;
			assert_eq!(indices_start % 3, 0);
			indices_start / 3
		};
		out.triangles.extend(mesh.triangles.iter());
		while out.triangles.len() % 3 != 0 {
			out.triangles.push(CompressedIndices(0));
		}

		out.meshlets.extend(mesh.meshlets.iter().map(|m| MeshletData {
			draw_vertex_offset: MeshletOffset::new(
				draw_start + m.draw_vertex_offset.start(),
				m.draw_vertex_offset.len(),
			),
			triangle_offset: MeshletOffset::new(triangle_start + m.triangle_offset.start(), m.triangle_offset.len()),
		}))
	}

	out
}
