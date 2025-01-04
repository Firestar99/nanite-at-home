use crate::meshlet::mesh::MeshletReader;
use rayon::prelude::*;
use rkyv::{Archive, Deserialize, Serialize};
use space_asset_disk_shader::meshlet::indices::{CompressedIndices, INDICES_PER_WORD};
use space_asset_disk_shader::meshlet::mesh::MeshletData;
use space_asset_disk_shader::meshlet::offset::MeshletOffset;
use space_asset_disk_shader::meshlet::vertex::DrawVertex;

#[derive(Clone, Debug, Default, Archive, Serialize, Deserialize)]
pub struct LodMesh {
	pub meshlets: Vec<MeshletData>,
	pub draw_vertices: Vec<DrawVertex>,
	pub triangles: Vec<CompressedIndices>,
}

impl LodMesh {
	pub fn meshlet(&self, index: usize) -> MeshletReader<Self> {
		MeshletReader {
			data: self.meshlets[index],
			mesh: self,
		}
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
		let list = par_iter.into_par_iter().collect_vec_list();
		let mut mesh = LodMesh {
			meshlets: Vec::with_capacity(list.iter().flatten().map(|a| a.meshlets.len()).sum()),
			draw_vertices: Vec::with_capacity(list.iter().flatten().map(|a| a.draw_vertices.len()).sum()),
			triangles: Vec::with_capacity(list.iter().flatten().map(|a| a.triangles.len()).sum()),
		};
		for mut e in list.into_iter().flatten() {
			mesh.append(&mut e);
		}
		mesh
	}
}

impl FromIterator<LodMesh> for LodMesh {
	fn from_iter<T: IntoIterator<Item = LodMesh>>(iter: T) -> Self {
		let mut mesh = LodMesh::default();
		for mut e in iter.into_iter() {
			mesh.append(&mut e);
		}
		mesh
	}
}
