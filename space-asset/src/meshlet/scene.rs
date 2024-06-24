#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Archive, Serialize, Deserialize)]
	pub struct MeshletSceneDisk {
		pub mesh2instance: Vec<MeshletMesh2InstanceDisk>,
	}
}

#[cfg(feature = "disk")]
pub use disk::*;
