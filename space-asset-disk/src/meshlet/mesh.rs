use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk_shader::material::pbr::PbrVertex;
use space_asset_disk_shader::meshlet::indices::{triangle_indices_load, CompressedIndices};
use space_asset_disk_shader::meshlet::vertex::{DrawVertex, MaterialVertexId};
use std::ops::{Deref, Index};

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

	pub fn meshlet(&self, index: usize) -> MeshletReader {
		assert!(
			index < self.meshlets.len(),
			"meshlet index out of bounds: the len is {} but the index is {}",
			self.meshlets.len(),
			index
		);
		MeshletReader {
			data: self.meshlets[index],
			mesh: self,
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MeshletReader<'a> {
	pub data: MeshletData,
	pub mesh: &'a MeshletMeshDisk,
}

impl<'a> Deref for MeshletReader<'a> {
	type Target = MeshletData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<'a, T> AsRef<T> for MeshletReader<'a>
where
	T: ?Sized,
	<MeshletReader<'a> as Deref>::Target: AsRef<T>,
{
	fn as_ref(&self) -> &T {
		self.deref().as_ref()
	}
}

impl<'a> MeshletReader<'a> {
	pub fn load_draw_vertex(&self, index: usize) -> DrawVertex {
		let len = self.data.draw_vertex_offset.len();
		assert!(
			index < len,
			"index out of bounds: the len is {len} but the index is {index}"
		);
		let global_index = self.data.draw_vertex_offset.start() + index;
		*self.mesh.draw_vertices.index(global_index)
	}

	pub fn load_triangle(&self, triangle: usize) -> UVec3 {
		let len = self.data.triangle_offset.len();
		assert!(
			triangle < len,
			"index out of bounds: the len is {len} but the index is {triangle}"
		);
		triangle_indices_load(self, &(), triangle, |_, i| self.mesh.triangles[i])
	}

	pub fn load_pbr_material_vertex(&self, index: MaterialVertexId) -> PbrVertex {
		self.mesh.pbr_material_vertices[index.0 as usize]
	}
}

pub use space_asset_disk_shader::meshlet::mesh::*;
