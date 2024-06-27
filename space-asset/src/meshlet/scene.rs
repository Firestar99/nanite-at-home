#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
	use rkyv::{AlignedVec, Archive, Deserialize, Serialize};
	use std::io;
	use std::io::{Read, Write};

	#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instances: Vec<MeshletMesh2InstanceDisk>,
	}

	impl MeshletSceneDisk {
		#[profiling::function]
		pub fn serialize(&self) -> AlignedVec {
			// only a very little scratch space is needed
			rkyv::to_bytes::<_, 1024>(self).unwrap()
		}

		#[profiling::function]
		pub fn serialize_compress_to(&self, write: impl Write) -> io::Result<()> {
			self.compress_to(write, self.serialize())
		}

		#[profiling::function]
		fn compress_to(&self, write: impl Write, vec: AlignedVec) -> io::Result<()> {
			zstd::stream::copy_encode(vec.as_slice(), write, 0)
		}
	}

	pub struct LoadedMeshletSceneDisk {
		archive: Vec<u8>,
	}

	impl LoadedMeshletSceneDisk {
		pub fn deserialize(&self) -> &ArchivedMeshletSceneDisk {
			unsafe { rkyv::archived_root::<MeshletSceneDisk>(&self.archive) }
		}

		/// Deserialize and decompress from a readable data stream, like a file.
		///
		/// # Safety
		/// Must be a valid datastream retrieved from [`MeshletSceneDisk::serialize_compress_to`]
		#[profiling::function]
		pub unsafe fn deserialize_decompress(read: impl Read) -> io::Result<Self> {
			Ok(Self {
				archive: zstd::stream::decode_all(read)?,
			})
		}
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
