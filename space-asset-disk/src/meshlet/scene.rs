use crate::material::pbr::PbrMaterialDisk;
use crate::meshlet::instance::MeshletInstanceDisk;
use crate::meshlet::mesh::MeshletMeshDisk;
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
	pub pbr_materials: Vec<PbrMaterialDisk>,
	pub meshes: Vec<MeshletMeshDisk>,
	pub instances: Vec<MeshletInstanceDisk>,
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

	#[profiling::function]
	pub fn load(&self) -> io::Result<LoadedMeshletScene> {
		let path = self.absolute_path()?;
		let mut file = fs::File::open(&path)?;

		let capacity = path.metadata()?.len() as usize + 1;
		let mut vec = AlignedVec::with_capacity(capacity);
		unsafe {
			vec.set_len(capacity);
		}

		let mut bytes_read = 0;
		loop {
			profiling::scope!("File::read");
			match file.read(&mut vec[bytes_read..]) {
				Ok(e) => {
					bytes_read += e;
					if e == 0 {
						break;
					}
				}
				Err(err) if err.kind() == io::ErrorKind::Interrupted => (),
				Err(err) => Err(err)?,
			}
		}
		unsafe {
			vec.set_len(bytes_read);
		}
		Ok(LoadedMeshletScene { archive: vec })
	}
}

impl LoadedMeshletScene {
	pub fn root(&self) -> &ArchivedMeshletSceneDisk {
		unsafe { rkyv::archived_root::<MeshletSceneDisk>(&self.archive) }
	}
}
