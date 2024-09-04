mod gpu {
	use crate::meshlet::instance::MeshletInstance;
	use crate::meshlet::mesh::MeshletMesh;
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::{Buffer, Desc, DescRef, DescStructRef};

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	pub struct MeshletMesh2Instance<R: DescRef, RR: DescStructRef + 'static> {
		pub mesh: Desc<R, Buffer<MeshletMesh<RR>>>,
		pub instances: Desc<R, Buffer<[MeshletInstance]>>,
	}
}

pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::instance::MeshletInstance;
	use crate::meshlet::mesh::MeshletMeshDisk;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletMesh2InstanceDisk {
		pub mesh: MeshletMeshDisk,
		pub instances: Vec<MeshletInstance>,
	}
}

#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::material::pbr::PbrMaterial;
	use crate::meshlet::mesh2instance::{ArchivedMeshletMesh2InstanceDisk, MeshletMesh2Instance};
	use crate::uploader::{deserialize_infallible, UploadError, Uploader};
	use std::future::Future;
	use std::ops::Deref;
	use vulkano::Validated;
	use vulkano_bindless::descriptor::{Strong, RC};

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

	impl ArchivedMeshletMesh2InstanceDisk {
		pub fn upload<'a>(
			&'a self,
			uploader: &'a Uploader,
			pbr_materials: &'a Vec<PbrMaterial<RC>>,
		) -> impl Future<Output = Result<MeshletMesh2InstanceCpu, Validated<UploadError>>> + 'a {
			let mesh = self.mesh.upload(uploader, pbr_materials);
			let instances = uploader.upload_buffer_iter(self.instances.iter().map(deserialize_infallible));
			async {
				let mesh = uploader.upload_buffer_data(mesh.await?.to_strong());
				Ok(MeshletMesh2InstanceCpu {
					mesh2instance: MeshletMesh2Instance {
						mesh: mesh.await?.into(),
						instances: instances.await?.into(),
					},
					num_meshlets: self.mesh.meshlets.len() as u32,
				})
			}
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
