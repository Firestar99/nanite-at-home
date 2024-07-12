#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
	use rkyv::ser::serializers::{
		AllocScratch, CompositeSerializer, CompositeSerializerError, FallbackScratch, HeapScratch, SharedSerializeMap,
		WriteSerializer,
	};
	use rkyv::ser::Serializer;
	use rkyv::{Archive, Deserialize, Serialize};
	use std::io;
	use std::io::{BufWriter, Write};

	#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instances: Vec<MeshletMesh2InstanceDisk>,
	}

	impl MeshletSceneDisk {
		#[profiling::function]
		pub fn serialize_to(&self, write: impl Write) -> io::Result<()> {
			let mut serializer = CompositeSerializer::new(
				WriteSerializer::new(BufWriter::with_capacity(128 * 1024, write)),
				FallbackScratch::<HeapScratch<1024>, AllocScratch>::default(),
				SharedSerializeMap::default(),
			);
			serializer.serialize_value(self).map_err(|err| match err {
				CompositeSerializerError::SerializerError(e) => e,
				CompositeSerializerError::ScratchSpaceError(e) => Err(e).unwrap(),
				CompositeSerializerError::SharedError(e) => Err(e).unwrap(),
			})?;
			Ok(())
		}
	}

	impl ArchivedMeshletSceneDisk {
		/// Deserialize from a byte slice
		///
		/// # Safety
		/// Must be a valid datastream retrieved from [`MeshletSceneDisk::serialize_to`]
		pub unsafe fn deserialize(archive: &[u8]) -> &Self {
			unsafe { rkyv::archived_root::<MeshletSceneDisk>(archive) }
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
