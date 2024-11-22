use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk_shader::material::pbr::PbrVertex;
use space_asset_disk_shader::meshlet::indices::CompressedIndices;
use space_asset_disk_shader::meshlet::vertex::DrawVertex;

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletMeshDisk {
	pub meshlets: Vec<MeshletData>,
	pub draw_vertices: Vec<DrawVertex>,
	pub triangles: Vec<CompressedIndices>,
	pub pbr_material_vertices: Vec<PbrVertex>,
	pub pbr_material_id: Option<u32>,
}

pub use space_asset_disk_shader::meshlet::mesh::*;
