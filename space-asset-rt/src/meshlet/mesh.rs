use crate::material::pbr::PbrMaterials;
use crate::upload_traits::ToStrong;
use crate::uploader::{deserialize_infallible, Uploader};
use rust_gpu_bindless::descriptor::{RCDescExt, RC};
use rust_gpu_bindless_shaders::descriptor::Strong;
use space_asset_disk::meshlet::mesh::ArchivedMeshletMeshDisk;
use space_asset_shader::meshlet::mesh::MeshletMesh;
use std::future::Future;

impl ToStrong for MeshletMesh<RC> {
	type StrongType = MeshletMesh<Strong>;

	fn to_strong(&self) -> Self::StrongType {
		MeshletMesh {
			meshlets: self.meshlets.to_strong(),
			draw_vertices: self.draw_vertices.to_strong(),
			triangles: self.triangles.to_strong(),
			num_meshlets: self.num_meshlets,
			pbr_material: self.pbr_material.to_strong(),
			pbr_material_vertices: self.pbr_material_vertices.to_strong(),
		}
	}
}

pub fn upload_mesh<'a>(
	this: &'a ArchivedMeshletMeshDisk,
	uploader: &'a Uploader,
	pbr_materials: &'a PbrMaterials<'a>,
) -> impl Future<Output = anyhow::Result<MeshletMesh<RC>>> + 'a {
	profiling::scope!("upload_mesh");
	let meshlets = uploader.upload_buffer_iter("meshlets", this.lod_mesh.meshlets.iter().map(deserialize_infallible));
	let draw_vertices = uploader.upload_buffer_iter(
		"draw_vertices",
		this.lod_mesh.draw_vertices.iter().map(deserialize_infallible),
	);
	let triangles =
		uploader.upload_buffer_iter("triangles", this.lod_mesh.triangles.iter().map(deserialize_infallible));
	let pbr_material_vertices = uploader.upload_buffer_iter(
		"pbr_material_vertices",
		this.pbr_material_vertices.iter().map(deserialize_infallible),
	);
	let pbr_material_id: Option<u32> = deserialize_infallible(&this.pbr_material_id);
	async move {
		Ok(MeshletMesh {
			meshlets: meshlets.await?,
			draw_vertices: draw_vertices.await?,
			triangles: triangles.await?,
			num_meshlets: this.lod_mesh.meshlets.len() as u32,
			pbr_material: pbr_material_id
				.map_or(pbr_materials.default_pbr_material, |i| {
					pbr_materials.pbr_materials.get(i as usize).unwrap()
				})
				.clone(),
			pbr_material_vertices: pbr_material_vertices.await?,
		})
	}
}
