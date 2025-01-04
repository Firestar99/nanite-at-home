use crate::material::pbr::{default_pbr_material, upload_pbr_material, PbrMaterials};
use crate::meshlet::mesh::upload_mesh;
use crate::upload_traits::ToStrong;
use crate::uploader::{deserialize_infallible, UploadError, Uploader};
use futures::future::join_all;
use rayon::prelude::*;
use space_asset_disk::meshlet::scene::ArchivedMeshletSceneDisk;
use space_asset_disk::range::{ArchivedRangeU32, RangeU32};
use space_asset_shader::affine_transform::AffineTransform;
use space_asset_shader::material::pbr::PbrMaterial;
use space_asset_shader::meshlet::instance::MeshInstance;
use space_asset_shader::meshlet::mesh::MeshletMesh;
use space_asset_shader::meshlet::scene::MeshletScene;
use vulkano::Validated;
use vulkano_bindless::descriptor::{RCDesc, RCDescExt, RC};
use vulkano_bindless_shaders::descriptor::{Buffer, Strong};

#[derive(Clone, Debug)]
pub struct MeshletSceneCpu {
	pub scene: RCDesc<Buffer<MeshletScene<Strong>>>,
	pub num_instances: u32,
}

pub async fn upload_scene(
	this: &ArchivedMeshletSceneDisk,
	uploader: &Uploader,
) -> Result<MeshletSceneCpu, Validated<UploadError>> {
	profiling::scope!("ArchivedMeshletSceneDisk::upload");

	let pbr_materials: Vec<PbrMaterial<RC>> = {
		profiling::scope!("material upload");
		join_all(
			this.pbr_materials
				.par_iter()
				.map(|mat| upload_pbr_material(mat, uploader))
				.collect::<Vec<_>>(),
		)
		.await
		.into_iter()
		.collect::<Result<_, _>>()?
	};
	let pbr_materials = PbrMaterials {
		pbr_materials: &pbr_materials,
		default_pbr_material: &default_pbr_material(uploader),
	};

	let meshes: Vec<MeshletMesh<RC>> = {
		profiling::scope!("mesh upload");
		join_all(
			this.meshes
				.par_iter()
				.map(|mesh| upload_mesh(&mesh, uploader, &pbr_materials))
				.collect::<Vec<_>>(),
		)
		.await
		.into_iter()
		.collect::<Result<_, _>>()?
	};

	let instances = {
		profiling::scope!("instances upload");
		uploader
			.upload_buffer_iter(this.instances.iter().map(|instance| MeshInstance {
				transform: AffineTransform::new(instance.transform),
				mesh_ids: deserialize_infallible::<ArchivedRangeU32, RangeU32>(&instance.mesh_ids),
			}))
			.await?
	};

	let scene = {
		profiling::scope!("scene upload");
		let meshes_buffer = uploader
			.upload_buffer_iter(meshes.iter().map(|m| m.to_strong()))
			.await?;
		uploader
			.upload_buffer_data(MeshletScene::<Strong> {
				meshes: meshes_buffer.to_strong(),
				instances: instances.to_strong(),
			})
			.await?
	};
	Ok(MeshletSceneCpu {
		scene,
		num_instances: instances.len() as u32,
	})
}
