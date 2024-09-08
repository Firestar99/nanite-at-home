use crate::upload_traits::ToStrong;
use crate::uploader::{deserialize_infallible, UploadError, Uploader};
use space_asset_disk::meshlet::mesh::ArchivedMeshletMeshDisk;
use space_asset_shader::material::pbr::PbrMaterial;
use space_asset_shader::meshlet::mesh::MeshletMesh;
use std::future::Future;
use vulkano::Validated;
use vulkano_bindless::descriptor::{RCDescExt, RC};
use vulkano_bindless_shaders::descriptor::Strong;

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
	pbr_materials: &'a [PbrMaterial<RC>],
) -> impl Future<Output = Result<MeshletMesh<RC>, Validated<UploadError>>> + 'a {
	let meshlets = uploader.upload_buffer_iter(this.meshlets.iter().map(deserialize_infallible));
	let draw_vertices = uploader.upload_buffer_iter(this.draw_vertices.iter().map(deserialize_infallible));
	let triangles = uploader.upload_buffer_iter(this.triangles.iter().map(deserialize_infallible));
	let pbr_material_vertices =
		uploader.upload_buffer_iter(this.pbr_material_vertices.iter().map(deserialize_infallible));
	async {
		Ok(MeshletMesh {
			meshlets: meshlets.await?.into(),
			draw_vertices: draw_vertices.await?.into(),
			triangles: triangles.await?.into(),
			num_meshlets: this.meshlets.len() as u32,
			pbr_material: pbr_materials.get(this.pbr_material_id as usize).unwrap().clone(),
			pbr_material_vertices: pbr_material_vertices.await?.into(),
		})
	}
}
