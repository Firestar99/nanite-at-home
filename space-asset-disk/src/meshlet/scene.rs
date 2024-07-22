#[cfg(feature = "disk")]
mod disk {
	use crate::material::pbr::PbrMaterialDisk;
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
	use rkyv::ser::serializers::{
		AllocScratch, CompositeSerializer, CompositeSerializerError, FallbackScratch, HeapScratch, SharedSerializeMap,
		WriteSerializer,
	};
	use rkyv::ser::Serializer;
	use rkyv::{AlignedVec, Archive, Deserialize, Serialize};
	use std::io::{BufWriter, Read, Write};
	use std::path::PathBuf;
	use std::{fs, io};

	#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instances: Vec<MeshletMesh2InstanceDisk>,
		pub pbr_materials: Vec<PbrMaterialDisk>,
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
				CompositeSerializerError::ScratchSpaceError(e) => panic!("{:?}", e),
				CompositeSerializerError::SharedError(e) => panic!("{:?}", e),
			})?;
			Ok(())
		}
	}

	#[derive(Copy, Clone, Debug)]
	pub struct MeshletSceneFile<'a> {
		path: &'a str,
	}

	pub struct LoadedMeshletScene {
		archive: AlignedVec,
	}

	pub const EXPORT_FOLDER_NAME: &str = "assets";

	impl<'a> MeshletSceneFile<'a> {
		/// Create a new MeshletSceneFile with a path relative to the export directory.
		///
		/// # Safety
		/// File must contain a valid datastream retrieved from [`MeshletSceneDisk::serialize_to`]
		pub const unsafe fn new(path: &'a str) -> Self {
			Self { path }
		}

		pub fn absolute_path(&self) -> io::Result<PathBuf> {
			let exe = std::env::current_exe()?.canonicalize()?;
			let exe_dir = exe
				.parent()
				.ok_or(io::Error::new(io::ErrorKind::NotFound, "executable's dir not found"))?;
			let mut file = PathBuf::from(exe_dir);
			file.push(EXPORT_FOLDER_NAME);
			file.push(self.path);
			Ok(file)
		}

		pub fn load(&self) -> io::Result<LoadedMeshletScene> {
			let path = self.absolute_path()?;
			let mut file = fs::File::open(&path)?;

			let capacity = path.metadata()?.len() as usize + 1;
			let mut vec = AlignedVec::with_capacity(capacity);
			vec.resize(capacity, 0);

			loop {
				match file.read(&mut vec) {
					Ok(e) => {
						vec.resize(e, 0);
						break;
					}
					Err(err) if err.kind() == io::ErrorKind::Interrupted => (),
					Err(err) => Err(err)?,
				}
			}
			Ok(LoadedMeshletScene { archive: vec })
		}
	}

	impl LoadedMeshletScene {
		pub fn root(&self) -> &ArchivedMeshletSceneDisk {
			unsafe { rkyv::archived_root::<MeshletSceneDisk>(&self.archive) }
		}
	}
}

#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::material::pbr::PbrMaterial;
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceCpu;
	use crate::meshlet::scene::ArchivedMeshletSceneDisk;
	use crate::uploader::{UploadError, Uploader};
	use futures::future::join_all;
	use rayon::prelude::*;
	use vulkano::Validated;
	use vulkano_bindless::descriptor::RC;

	pub struct MeshletSceneCpu {
		pub mesh2instances: Vec<MeshletMesh2InstanceCpu>,
	}

	impl ArchivedMeshletSceneDisk {
		pub async fn upload(&self, uploader: &Uploader) -> Result<MeshletSceneCpu, Validated<UploadError>> {
			profiling::scope!("ArchivedMeshletSceneDisk::upload");

			let pbr_materials: Vec<PbrMaterial<RC>> = {
				profiling::scope!("material upload");
				join_all(
					self.pbr_materials
						.par_iter()
						.map(|mat| mat.upload(uploader))
						.collect::<Vec<_>>(),
				)
				.await
				.into_iter()
				.collect::<Result<_, _>>()?
			};

			let mesh2instances = {
				profiling::scope!("mesh upload");
				join_all(
					self.mesh2instances
						.par_iter()
						.map(|m2i| m2i.upload(uploader, &pbr_materials))
						.collect::<Vec<_>>(),
				)
				.await
				.into_iter()
				.collect::<Result<_, _>>()?
			};

			Ok(MeshletSceneCpu { mesh2instances })
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
