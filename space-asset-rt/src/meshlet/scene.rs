use crate::material::pbr::{default_pbr_material, upload_pbr_material, PbrMaterials};
use crate::meshlet::mesh2instance::{upload_mesh_2_instance, MeshletMesh2InstanceCpu};
use crate::uploader::{UploadError, Uploader};
use futures::future::join_all;
use rayon::prelude::*;
use space_asset_disk::meshlet::scene::ArchivedMeshletSceneDisk;
use space_asset_shader::material::pbr::PbrMaterial;
use vulkano::Validated;
use vulkano_bindless::descriptor::RC;

pub struct MeshletSceneCpu {
	pub mesh2instances: Vec<MeshletMesh2InstanceCpu>,
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

	let mesh2instances = {
		profiling::scope!("mesh upload");
		join_all(
			this.mesh2instances
				.par_iter()
				.map(|m2i| upload_mesh_2_instance(m2i, uploader, &pbr_materials))
				.collect::<Vec<_>>(),
		)
		.await
		.into_iter()
		.collect::<Result<_, _>>()?
	};

	Ok(MeshletSceneCpu { mesh2instances })
}
