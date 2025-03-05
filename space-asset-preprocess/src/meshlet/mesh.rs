use crate::meshlet::lod_mesh::LodMesh;
use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk::material::pbr::PbrVertex;
use space_asset_disk::meshlet::indices::triangle_indices_load;
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::stats::SourceMeshStats;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use std::ops::{Deref, DerefMut, Index};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletMesh {
	pub lod_mesh: LodMesh,
	pub pbr_material_vertices: Vec<PbrVertex>,
	pub pbr_material_id: Option<u32>,
	pub stats: SourceMeshStats,
}

impl Deref for MeshletMesh {
	type Target = LodMesh;

	fn deref(&self) -> &Self::Target {
		&self.lod_mesh
	}
}

impl DerefMut for MeshletMesh {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lod_mesh
	}
}

impl AsRef<LodMesh> for MeshletMesh {
	fn as_ref(&self) -> &LodMesh {
		&self.lod_mesh
	}
}

impl MeshletMesh {
	pub fn meshlet(&self, index: usize) -> MeshletReader<Self> {
		MeshletReader {
			data: self.meshlets[index],
			mesh: self,
		}
	}

	pub fn to_meshlet_mesh_disk(self) -> anyhow::Result<MeshletMeshDisk> {
		Ok(MeshletMeshDisk {
			meshlets: self.lod_mesh.meshlets,
			draw_vertices: self.lod_mesh.draw_vertices,
			triangles: self.lod_mesh.triangles,
			pbr_material_vertices: self.pbr_material_vertices,
			pbr_material_id: self.pbr_material_id,
			stats: self.stats,
		})
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

impl<'a> MeshletReader<'a, MeshletMesh> {
	pub fn load_pbr_material_vertex(&self, index: MaterialVertexId) -> PbrVertex {
		self.mesh.pbr_material_vertices[index.0 as usize]
	}
}
