use crate::meshlet::mesh::upload_mesh;
use crate::upload_traits::ToStrong;
use crate::uploader::{deserialize_infallible, UploadError, Uploader};
use space_asset_disk::meshlet::instance::MeshletInstanceDisk;
use space_asset_disk::meshlet::mesh2instance::ArchivedMeshletMesh2InstanceDisk;
use space_asset_shader::affine_transform::AffineTransform;
use space_asset_shader::material::pbr::PbrMaterial;
use space_asset_shader::meshlet::instance::MeshletInstance;
use space_asset_shader::meshlet::mesh2instance::MeshletMesh2Instance;
use std::future::Future;
use std::ops::Deref;
use vulkano::Validated;
use vulkano_bindless::descriptor::RC;
use vulkano_bindless_shaders::descriptor::Strong;

pub struct MeshletMesh2InstanceCpu {
	pub mesh2instance: MeshletMesh2Instance<RC, Strong>,
	pub num_meshlets: u32,
}

impl Deref for MeshletMesh2InstanceCpu {
	type Target = MeshletMesh2Instance<RC, Strong>;

	fn deref(&self) -> &Self::Target {
		&self.mesh2instance
	}
}

pub fn upload_mesh_2_instance<'a>(
	this: &'a ArchivedMeshletMesh2InstanceDisk,
	uploader: &'a Uploader,
	pbr_materials: &'a [PbrMaterial<RC>],
) -> impl Future<Output = Result<MeshletMesh2InstanceCpu, Validated<UploadError>>> + 'a {
	let mesh = upload_mesh(&this.mesh, uploader, pbr_materials);
	let instances = uploader.upload_buffer_iter(this.instances.iter().map(|a| {
		MeshletInstance::new(AffineTransform::new(
			deserialize_infallible::<_, MeshletInstanceDisk>(a).transform,
		))
	}));
	async {
		let mesh = uploader.upload_buffer_data(mesh.await?.to_strong());
		Ok(MeshletMesh2InstanceCpu {
			mesh2instance: MeshletMesh2Instance {
				mesh: mesh.await?.into(),
				instances: instances.await?.into(),
			},
			num_meshlets: this.mesh.meshlets.len() as u32,
		})
	}
}
