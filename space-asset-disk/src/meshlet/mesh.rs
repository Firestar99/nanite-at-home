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
	/// Indices to `meshlets` to only access the meshlets corresponding to a certain LOD level. Lod level N meshlets are
	/// in the slice of `meshlets[lod_ranges[N]..lod_ranges[N+1]]`, meaning that lod_ranges is always one longer than
	/// the lowest Lod level of the model.
	pub lod_ranges: Vec<u32>,
}

impl MeshletMeshDisk {
	pub fn lod_levels(&self) -> u32 {
		self.lod_ranges.len() as u32 - 1
	}
}

pub use space_asset_disk_shader::meshlet::mesh::*;
