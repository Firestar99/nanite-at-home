use crate::meshlet::lod_tree_gen::indices::{IndexPair, MeshletId};
use crate::meshlet::lod_tree_gen::sorted_smallvec::SortedSmallVec;
use smallvec::SmallVec;
use space_asset_disk::meshlet::mesh::MeshletMeshDisk;
use space_asset_disk::meshlet::vertex::MaterialVertexId;
use space_asset_disk::meshlet::MESHLET_MAX_TRIANGLES;
use static_assertions::const_assert_eq;
use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::size_of;

#[derive(Clone, Debug, Default)]
pub struct BorderTracker {
	/// Compressed Sparse Row `xadj` like METIS
	xadj: Vec<i32>,
	/// Compressed Sparse Row `adjncy` like METIS, adjacency indices are sorted
	adjncy: Vec<i32>,
	/// Compressed Sparse Row `adjncy` like METIS,
	/// but instead of containing adjacency indices it contains indices into `borders`
	adjncy_border_index: Vec<u32>,
	/// contains all borders
	borders: Vec<Border>,
}

#[derive(Clone, Debug, Default)]
pub struct Border {
	/// edge count looks like a normal distribution
	/// small models: will have up to 4-6 edges per border
	/// large models: 90% will be <= 10 edges
	/// we use 11 edges to nicely align to 12 * 8 bytes
	pub edges: SmallVec<[IndexPair<MaterialVertexId>; 11]>,
}
const_assert_eq!(size_of::<Border>(), 8 * 12);

impl BorderTracker {
	pub fn get_connected_meshlets(&self, meshlet: MeshletId) -> impl Iterator<Item = MeshletId> + '_ {
		let adjncy_range = self.xadj[*meshlet as usize] as usize..self.xadj[*meshlet as usize + 1] as usize;
		self.adjncy[adjncy_range].iter().map(|i| MeshletId(*i as u32))
	}

	pub fn get_border(&self, meshlets: IndexPair<MeshletId>) -> Option<&Border> {
		let adjncy_range = self.xadj[*meshlets.0 as usize] as usize..self.xadj[*meshlets.0 as usize + 1] as usize;
		let adjncy_index = adjncy_range.start + self.adjncy[adjncy_range].binary_search(&(*meshlets.1 as i32)).ok()?;
		Some(&self.borders[self.adjncy_border_index[adjncy_index] as usize])
	}

	pub fn from_meshlet_mesh(mesh: &MeshletMeshDisk) -> Self {
		// most Edges have only 1 meshlet, some 2, and in extremely rare cases >2
		// But we get a capacity of 4 for free, as SmallVec's heap alloc needs 16 bytes anyway
		const_assert_eq!(
			size_of::<SmallVec<[MeshletId; 1]>>(),
			size_of::<SmallVec<[MeshletId; 4]>>()
		);
		let mut edge_to_meshlets: HashMap<IndexPair<MaterialVertexId>, SmallVec<[MeshletId; 4]>>;
		// worst case we get 2 new edges per triangle, but around 1 edge per triangle is typical and HashMap
		// over allocates a bit anyway
		edge_to_meshlets = HashMap::with_capacity(mesh.meshlets.len() * MESHLET_MAX_TRIANGLES as usize);
		// Use a SmallVec of cap 6 for adjacency:
		// small models: 0..2
		// large models: usually 0..6, sometimes a few meshlets have significantly more
		let mut meshlet_adj: Vec<SortedSmallVec<[MeshletId; 6]>> = vec![SortedSmallVec::new(); mesh.meshlets.len()];
		for meshlet_id in 0..mesh.meshlets.len() {
			let meshlet = mesh.meshlet(meshlet_id);
			let meshlet_id = MeshletId(meshlet_id as u32);
			for triangle in 0..meshlet.triangle_offset.len() {
				let draw_indices = meshlet.load_triangle(triangle);
				let indices = draw_indices
					.to_array()
					.map(|i| meshlet.load_draw_vertex(i as usize).material_vertex_id);
				let edges = (0..3).map(|i| IndexPair::new(indices[i], indices[(i + 1) % 3]));
				for edge in edges {
					// it's not worth optimizing this search for (typically) at most 2 entries, just do a linear scan
					let vec = edge_to_meshlets.entry(edge).or_insert_with(SmallVec::new);
					if !vec.is_empty() {
						if vec.contains(&meshlet_id) {
							continue;
						}
						let x = &mut meshlet_adj[*meshlet_id as usize];
						for other_meshlet_id in vec.iter().copied() {
							x.insert(other_meshlet_id);
						}
						for other_meshlet_id in vec.iter().copied() {
							meshlet_adj[*other_meshlet_id as usize].insert(meshlet_id);
						}
					}
					vec.push(meshlet_id);
				}
			}
		}

		let mut xadj = Vec::with_capacity(mesh.meshlets.len() + 1);
		let mut adjncy = Vec::new();
		for i in 0..mesh.meshlets.len() {
			xadj.push(adjncy.len() as i32);
			adjncy.extend(meshlet_adj[i].iter().map(|id| id.0 as i32))
		}
		xadj.push(adjncy.len() as i32);
		assert_eq!(xadj.len(), mesh.meshlets.len() + 1);
		drop(meshlet_adj);

		let mut borders = Vec::with_capacity(adjncy.len() / 2);
		let mut adjncy_border_index = vec![!0; adjncy.len()];
		for meshlet_id in 0..xadj.len() - 1 {
			for adjncy_index in xadj[meshlet_id] as usize..xadj[meshlet_id + 1] as usize {
				let other_meshlet_id = adjncy[adjncy_index];
				if other_meshlet_id as usize > meshlet_id {
					let border_index = borders.len() as u32;
					borders.push(Border::default());
					adjncy_border_index[adjncy_index] = border_index;

					let other_range =
						xadj[other_meshlet_id as usize] as usize..xadj[other_meshlet_id as usize + 1] as usize;
					let other_adjncy_index =
						other_range.start + adjncy[other_range.clone()].binary_search(&(meshlet_id as i32)).unwrap();
					adjncy_border_index[other_adjncy_index] = border_index;
				}
			}
		}

		for (edge, meshlets) in edge_to_meshlets {
			for a in 0..meshlets.len() {
				for b in (a + 1)..meshlets.len() {
					let pair = IndexPair::new(meshlets[a], meshlets[b]);

					let adjncy_range = xadj[*pair.0 as usize] as usize..xadj[*pair.0 as usize + 1] as usize;
					let adjncy_index =
						adjncy_range.start + adjncy[adjncy_range].binary_search(&(*pair.1 as i32)).unwrap();
					let border = &mut borders[adjncy_border_index[adjncy_index] as usize];
					border.edges.push(edge);
				}
			}
		}

		BorderTracker {
			xadj,
			adjncy,
			adjncy_border_index,
			borders,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::gltf::Gltf;
	use crate::meshlet::process::process_meshlets;
	use std::path::Path;

	const LANTERN_GLTF_PATH: &str = concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../models/models/Lantern/glTF/Lantern.gltf"
	);

	#[test]
	fn test_lantern_gltf() -> anyhow::Result<()> {
		let gltf = Gltf::open(Path::new(LANTERN_GLTF_PATH))?;
		let scene = process_meshlets(&gltf)?;
		for mesh in scene.meshes {
			let tracker = BorderTracker::from_meshlet_mesh(&mesh);
			println!("xadj {:#?}", tracker.xadj);
			println!("adjncy {:#?}", tracker.adjncy);
			println!("adjncy_border_index {:#?}", tracker.adjncy_border_index);
			// println!("Borders {:#?}", tracker.borders)
		}
		Ok(())
	}
}
