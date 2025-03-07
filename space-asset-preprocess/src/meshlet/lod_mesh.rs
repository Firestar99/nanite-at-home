use crate::meshlet::lod_tree_gen::indices::MeshletId;
use crate::meshlet::mesh::MeshletReader;
use rayon::prelude::*;
use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk::meshlet::indices::{CompressedIndices, INDICES_PER_WORD};
use space_asset_disk::meshlet::mesh::MeshletData;
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::vertex::DrawVertex;

#[derive(Clone, Debug, Default, Archive, Serialize, Deserialize)]
pub struct LodMesh {
	pub meshlets: Vec<MeshletData>,
	pub draw_vertices: Vec<DrawVertex>,
	pub triangles: Vec<CompressedIndices>,
}

impl LodMesh {
	pub fn meshlet(&self, index: MeshletId) -> MeshletReader<Self> {
		MeshletReader {
			data: self.meshlets[index.0 as usize],
			mesh: self,
		}
	}

	pub fn meshlet_mut(&mut self, index: MeshletId) -> &mut MeshletData {
		&mut self.meshlets[index.0 as usize]
	}

	pub fn append(&mut self, other: &mut Self) {
		let draw_vertices_start = self.draw_vertices.len();
		let triangle_start = self.triangles.len() * INDICES_PER_WORD / 3;
		self.draw_vertices.append(&mut other.draw_vertices);
		self.triangles.append(&mut other.triangles);

		for m in &mut other.meshlets {
			m.draw_vertex_offset = MeshletOffset::new(
				draw_vertices_start + m.draw_vertex_offset.start(),
				m.draw_vertex_offset.len(),
			);
			m.triangle_offset = MeshletOffset::new(triangle_start + m.triangle_offset.start(), m.triangle_offset.len());
		}
		self.meshlets.append(&mut other.meshlets);
	}
}

impl AsRef<LodMesh> for LodMesh {
	fn as_ref(&self) -> &LodMesh {
		self
	}
}

impl FromParallelIterator<LodMesh> for LodMesh {
	fn from_par_iter<I>(par_iter: I) -> Self
	where
		I: IntoParallelIterator<Item = LodMesh>,
	{
		let mut mesh = Self::default();
		mesh.par_extend(par_iter);
		mesh
	}
}

impl ParallelExtend<LodMesh> for LodMesh {
	fn par_extend<I>(&mut self, par_iter: I)
	where
		I: IntoParallelIterator<Item = LodMesh>,
	{
		let list = par_iter.into_par_iter().collect_vec_list();
		self.meshlets
			.reserve(list.iter().flatten().map(|a| a.meshlets.len()).sum());
		self.draw_vertices
			.reserve(list.iter().flatten().map(|a| a.draw_vertices.len()).sum());
		self.triangles
			.reserve(list.iter().flatten().map(|a| a.triangles.len()).sum());
		for mut e in list.into_iter().flatten() {
			self.append(&mut e);
		}
	}
}

impl FromIterator<LodMesh> for LodMesh {
	fn from_iter<T: IntoIterator<Item = LodMesh>>(iter: T) -> Self {
		let mut mesh = LodMesh::default();
		mesh.extend(iter);
		mesh
	}
}

impl Extend<LodMesh> for LodMesh {
	fn extend<T: IntoIterator<Item = LodMesh>>(&mut self, iter: T) {
		for mut e in iter.into_iter() {
			self.append(&mut e);
		}
	}
}
