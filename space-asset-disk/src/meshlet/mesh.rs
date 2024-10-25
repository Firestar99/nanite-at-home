use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk_shader::material::pbr::PbrVertex;
use space_asset_disk_shader::meshlet::indices::triangle_indices_load;
use space_asset_disk_shader::meshlet::vertex::{DrawVertex, MaterialVertexId};
use std::ops::{Deref, DerefMut, Index};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletMeshDisk {
	pub lod_mesh: LodMesh,
	pub pbr_material_vertices: Vec<PbrVertex>,
	pub pbr_material_id: Option<u32>,
	/// Indices to `meshlets` to only access the meshlets corresponding to a certain LOD level. Lod level N meshlets are
	/// in the slice of `meshlets[lod_ranges[N]..lod_ranges[N+1]]`, meaning that lod_ranges is always one longer than
	/// the lowest Lod level of the model.
	pub lod_ranges: Vec<u32>,
}

impl Deref for MeshletMeshDisk {
	type Target = LodMesh;

	fn deref(&self) -> &Self::Target {
		&self.lod_mesh
	}
}

impl DerefMut for MeshletMeshDisk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lod_mesh
	}
}

impl AsRef<LodMesh> for MeshletMeshDisk {
	fn as_ref(&self) -> &LodMesh {
		&self.lod_mesh
	}
}

impl MeshletMeshDisk {
	pub fn lod_levels(&self) -> u32 {
		self.lod_ranges.len() as u32 - 1
	}

	pub fn meshlet(&self, index: usize) -> MeshletReader<Self> {
		MeshletReader {
			data: self.meshlets[index],
			mesh: self,
		}
	}

	pub fn append_lod_level(&mut self, mesh: &mut LodMesh) {
		self.lod_mesh.append(mesh);
		self.lod_ranges.push(self.lod_mesh.meshlets.len() as u32);
	}
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MeshletReader<'a, M: AsRef<LodMesh>> {
	pub data: MeshletData,
	pub mesh: &'a M,
}

impl<'a, M: AsRef<LodMesh>> Deref for MeshletReader<'a, M> {
	type Target = MeshletData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<'a, T, M: AsRef<LodMesh>> AsRef<T> for MeshletReader<'a, M>
where
	T: ?Sized,
	<Self as Deref>::Target: AsRef<T>,
{
	fn as_ref(&self) -> &T {
		self.deref().as_ref()
	}
}

impl<'a, M: AsRef<LodMesh>> MeshletReader<'a, M> {
	pub fn load_draw_vertex(&self, index: usize) -> DrawVertex {
		let len = self.data.draw_vertex_offset.len();
		assert!(
			index < len,
			"index out of bounds: the len is {len} but the index is {index}"
		);
		let global_index = self.data.draw_vertex_offset.start() + index;
		*self.mesh.as_ref().draw_vertices.index(global_index)
	}

	pub fn load_triangle(&self, triangle: usize) -> UVec3 {
		let len = self.data.triangle_offset.len();
		assert!(
			triangle < len,
			"index out of bounds: the len is {len} but the index is {triangle}"
		);
		triangle_indices_load(self, &(), triangle, |_, i| self.mesh.as_ref().triangles[i])
	}
}

impl<'a> MeshletReader<'a, MeshletMeshDisk> {
	pub fn load_pbr_material_vertex(&self, index: MaterialVertexId) -> PbrVertex {
		self.mesh.pbr_material_vertices[index.0 as usize]
	}
}

use crate::meshlet::lod_mesh::LodMesh;
pub use space_asset_disk_shader::meshlet::mesh::*;
