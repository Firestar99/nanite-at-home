#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instances: Vec<MeshletMesh2InstanceDisk>,
	}
}

#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceCpu;
	use crate::meshlet::scene::ArchivedMeshletSceneDisk;
	use crate::uploader::{UploadError, Uploader};
	use futures::future::join_all;
	use rayon::prelude::*;
	use vulkano::Validated;

	pub struct MeshletSceneCpu {
		pub mesh2instances: Vec<MeshletMesh2InstanceCpu>,
	}

	impl ArchivedMeshletSceneDisk {
		pub async fn upload(&self, uploader: &Uploader) -> Result<MeshletSceneCpu, Validated<UploadError>> {
			profiling::scope!("ArchivedMeshletSceneDisk::upload");
			Ok(MeshletSceneCpu {
				mesh2instances: join_all(
					self.mesh2instances
						.par_iter()
						.map(|m2i| m2i.upload(uploader))
						.collect::<Vec<_>>(),
				)
				.await
				.into_iter()
				.collect::<Result<_, _>>()?,
			})
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
