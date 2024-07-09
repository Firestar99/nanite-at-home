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
	use std::io::{BufWriter, Read, Write};

	#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instances: Vec<MeshletMesh2InstanceDisk>,
	}

	impl MeshletSceneDisk {
		#[profiling::function]
		pub fn serialize_compress_to(&self, write: impl Write) -> io::Result<()> {
			let mut serializer = CompositeSerializer::new(
				WriteSerializer::new(BufWriter::with_capacity(
					zstd::stream::Encoder::<Vec<u8>>::recommended_input_size(),
					zstd::stream::Encoder::new(write, 0)?.auto_finish(),
				)),
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
