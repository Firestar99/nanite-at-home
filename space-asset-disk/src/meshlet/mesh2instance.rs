use crate::meshlet::instance::MeshletInstanceDisk;
use crate::meshlet::mesh::MeshletMeshDisk;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletMesh2InstanceDisk {
	pub mesh: MeshletMeshDisk,
	pub instances: Vec<MeshletInstanceDisk>,
}
