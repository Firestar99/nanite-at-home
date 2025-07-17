use crate::image::ImageStorage;
use crate::material::pbr::PbrMaterialDisk;
use crate::meshlet::instance::MeshletInstanceDisk;
use crate::meshlet::mesh::MeshletMeshDisk;
use crate::meshlet::stats::MeshletSceneStats;
use rkyv::api::serialize_using;
use rkyv::ser::Serializer;
use rkyv::ser::sharing::Share;
use rkyv::ser::writer::IoWriter;
use rkyv::util::{AlignedVec, with_arena};
use rkyv::{Archive, Deserialize, Serialize};
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;
use std::{fs, io};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletSceneDisk {
	pub image_storage: ImageStorage,
	pub pbr_materials: Vec<PbrMaterialDisk>,
	pub meshes: Vec<MeshletMeshDisk>,
	pub instances: Vec<MeshletInstanceDisk>,
	pub stats: MeshletSceneStats,
}

impl MeshletSceneDisk {
	pub fn serialize_to(&self, write: impl Write) -> io::Result<()> {
		profiling::function_scope!();
		with_arena(|arena| {
			let mut serializer = Serializer::new(
				IoWriter::new(BufWriter::with_capacity(128 * 1024, write)),
				arena.acquire(),
				Share::default(),
			);
			serialize_using::<_, rkyv::rancor::Panic>(self, &mut serializer).unwrap();
		});
		Ok(())
	}
}

#[derive(Copy, Clone, Debug)]
pub struct MeshletSceneFile<'a> {
	name: &'a str,
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
	pub const unsafe fn new(name: &'a str, path: &'a str) -> Self {
		Self { name, path }
	}

	pub fn name(&self) -> &'a str {
		self.name
	}

	pub fn path(&self) -> &'a str {
		self.path
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
		profiling::function_scope!();
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
		unsafe { rkyv::access_unchecked::<ArchivedMeshletSceneDisk>(&self.archive) }
	}
}
