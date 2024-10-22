use crate::meshlet::lod_tree_gen::indices::{IndexPair, MeshletId};
use crate::meshlet::lod_tree_gen::sorted_smallvec::SortedSmallVec;
use meshopt::{SimplifyOptions, VertexDataAdapter};
use smallvec::SmallVec;
use space_asset_disk::meshlet::indices::{
	triangle_indices_write, triangle_indices_write_capacity, CompressedIndices, INDICES_PER_WORD,
};
use space_asset_disk::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset_disk::meshlet::offset::MeshletOffset;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use static_assertions::const_assert_eq;
use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::{offset_of, size_of};

#[derive(Debug)]
pub struct BorderTracker<'a> {
	/// mesh
	mesh: &'a mut MeshletMeshDisk,
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

impl<'a> BorderTracker<'a> {
	pub fn get_connected_meshlets(&self, meshlet: MeshletId) -> impl Iterator<Item = MeshletId> + '_ {
		let adjncy_range = self.xadj[*meshlet as usize] as usize..self.xadj[*meshlet as usize + 1] as usize;
		self.adjncy[adjncy_range].iter().map(|i| MeshletId(*i as u32))
	}

	pub fn get_border(&self, meshlets: IndexPair<MeshletId>) -> Option<&Border> {
		let adjncy_range = self.xadj[*meshlets.0 as usize] as usize..self.xadj[*meshlets.0 as usize + 1] as usize;
		let adjncy_index = adjncy_range.start + self.adjncy[adjncy_range].binary_search(&(*meshlets.1 as i32)).ok()?;
		Some(&self.borders[self.adjncy_border_index[adjncy_index] as usize])
	}

	pub fn xadj(&self) -> &[i32] {
		&self.xadj
	}

	pub fn adjncy(&self) -> &[i32] {
		&self.adjncy
	}

	pub fn meshlets(&self) -> usize {
		self.xadj.len() - 1
	}

	#[profiling::function]
	pub fn from_meshlet_mesh(mesh: &'a mut MeshletMeshDisk) -> Self {
		// SmallVec: most Edges have only 1 meshlet, some 2, and in extremely rare cases >2
		// But we get a capacity of 4 for free, as SmallVec's heap alloc needs 16 bytes anyway
		const_assert_eq!(
			size_of::<SmallVec<[MeshletId; 1]>>(),
			size_of::<SmallVec<[MeshletId; 4]>>()
		);
		let mut edge_to_meshlets: HashMap<IndexPair<MaterialVertexId>, SmallVec<[MeshletId; 4]>>;
		// Use a SmallVec of cap 6 for adjacency:
		// small models: 0..2
		// large models: usually 0..6, sometimes a few meshlets have significantly more
		let mut meshlet_adj: Vec<SortedSmallVec<[MeshletId; 6]>>;
		{
			profiling::scope!("edge_to_meshlets meshlet_adj");
			// HashMap capacity: worst case we get 2 new edges per triangle, but around 1 edge per triangle is typical
			// and HashMap over allocates a bit anyway
			edge_to_meshlets = HashMap::with_capacity(mesh.meshlets.len() * MESHLET_MAX_TRIANGLES as usize);
			meshlet_adj = vec![SortedSmallVec::new(); mesh.meshlets.len()];
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
		}

		let mut xadj;
		let mut adjncy;
		{
			profiling::scope!("xadj adjncy");
			xadj = Vec::with_capacity(mesh.meshlets.len() + 1);
			adjncy = Vec::new();
			for i in 0..mesh.meshlets.len() {
				xadj.push(adjncy.len() as i32);
				adjncy.extend(meshlet_adj[i].iter().map(|id| id.0 as i32))
			}
			xadj.push(adjncy.len() as i32);
			assert_eq!(xadj.len(), mesh.meshlets.len() + 1);
			drop(meshlet_adj);
		}

		let mut borders;
		let mut adjncy_border_index;
		{
			profiling::scope!("adjncy_border_index");
			borders = Vec::with_capacity(adjncy.len() / 2);
			adjncy_border_index = vec![!0; adjncy.len()];
			for meshlet_id in 0..xadj.len() - 1 {
				for adjncy_index in xadj[meshlet_id] as usize..xadj[meshlet_id + 1] as usize {
					let other_meshlet_id = adjncy[adjncy_index];
					if other_meshlet_id as usize > meshlet_id {
						let border_index = borders.len() as u32;
						borders.push(Border::default());
						adjncy_border_index[adjncy_index] = border_index;

						let other_range =
							xadj[other_meshlet_id as usize] as usize..xadj[other_meshlet_id as usize + 1] as usize;
						let other_adjncy_index = other_range.start
							+ adjncy[other_range.clone()].binary_search(&(meshlet_id as i32)).unwrap();
						adjncy_border_index[other_adjncy_index] = border_index;
					}
				}
			}
		}

		{
			profiling::scope!("fill borders");
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
		}

		BorderTracker {
			mesh,
			xadj,
			adjncy,
			adjncy_border_index,
			borders,
		}
	}

	#[profiling::function]
	pub fn metis_partition(&self) -> Vec<i32> {
		let mut weights;
		{
			profiling::scope!("weights");
			weights = vec![0; self.adjncy.len()];
			for meshlet_id in 0..self.xadj.len() - 1 {
				for adjncy_index in self.xadj[meshlet_id] as usize..self.xadj[meshlet_id + 1] as usize {
					let border = &self.borders[self.adjncy_border_index[adjncy_index] as usize];
					weights[adjncy_index] = border.edges.len() as i32;
				}
			}
		}

		let mut partitions;
		{
			profiling::scope!("metis partitioning");
			let meshlet_merge_cnt = 4;
			let n_partitions = (self.meshlets() as i32 + meshlet_merge_cnt - 1) / meshlet_merge_cnt;
			partitions = vec![0; self.meshlets()];
			metis::Graph::new(1, n_partitions, self.xadj(), self.adjncy())
				.unwrap()
				.set_adjwgt(&weights)
				.part_kway(&mut partitions)
				.unwrap();
		}

		partitions
	}

	#[profiling::function]
	pub fn append_simplifed_meshlet_group(&mut self, meshlet_ids: &[MeshletId]) {
		let mut s_vertices;
		let mut s_indices;
		let mut s_remap;
		{
			profiling::scope!("simplify make mesh");
			let meshlets = meshlet_ids
				.iter()
				.map(|i| self.mesh.meshlet(i.0 as usize))
				.collect::<SmallVec<[_; 6]>>();
			let draw_vertex_cnt = meshlets.iter().map(|m| m.draw_vertex_offset.len()).sum();
			let triangle_cnt: usize = meshlets.iter().map(|m| m.triangle_offset.len()).sum();
			s_remap = HashMap::with_capacity(draw_vertex_cnt);
			s_vertices = Vec::with_capacity(draw_vertex_cnt);
			s_indices = Vec::with_capacity(triangle_cnt * 3);
			for m in meshlets {
				for tri in 0..m.triangle_offset.len() {
					for i in m.load_triangle(tri).to_array() {
						let draw = m.load_draw_vertex(i as usize);
						s_indices.push(*s_remap.entry(draw.material_vertex_id).or_insert({
							s_vertices.push(draw);
							s_vertices.len() as u32 - 1
						}));
					}
				}
			}
		}

		let mut s_vertex_lock;
		{
			profiling::scope!("simplify vertex_lock");
			s_vertex_lock = vec![false; s_vertices.len()];
			for id in meshlet_ids.iter().copied() {
				for oid in self.get_connected_meshlets(id) {
					if !meshlet_ids.contains(&oid) {
						for edge in &self.get_border(IndexPair::new(id, oid)).unwrap().edges {
							for vtx_id in edge.iter() {
								s_vertex_lock[s_remap[&vtx_id] as usize] = true;
							}
						}
					}
				}
			}
		}

		let target_count = s_indices.len() / 2;
		// let target_error = f32::lerp(0.01, 0.9, lod_level);
		let target_error = 0.01;

		{
			profiling::scope!("meshopt::simplify_with_locks");
			let adapter = VertexDataAdapter::new(
				bytemuck::cast_slice::<DrawVertex, u8>(&s_vertices),
				size_of::<DrawVertex>(),
				offset_of!(DrawVertex, position),
			)
			.unwrap();
			s_indices = meshopt::simplify_with_locks(
				&s_indices,
				&adapter,
				&s_vertex_lock,
				target_count,
				target_error,
				SimplifyOptions::empty(),
			);
		}

		{
			profiling::scope!("meshopt::optimize_vertex_fetch_in_place");
			let s_vertices_cnt = meshopt::optimize_vertex_fetch_in_place(&mut s_indices, &mut s_vertices);
			s_vertices.truncate(s_vertices_cnt);
		}

		{
			profiling::scope!("meshopt::optimize_vertex_cache_in_place");
			meshopt::optimize_vertex_cache_in_place(&mut s_indices, s_vertices.len());
		}

		let out;
		{
			profiling::scope!("meshopt::build_meshlets");
			let adapter = VertexDataAdapter::new(
				bytemuck::cast_slice::<DrawVertex, u8>(&s_vertices),
				size_of::<DrawVertex>(),
				offset_of!(DrawVertex, position),
			)
			.unwrap();
			let mut meshlets = meshopt::build_meshlets(
				&s_indices,
				&adapter,
				MESHLET_MAX_VERTICES as usize,
				MESHLET_MAX_TRIANGLES as usize,
				0.,
			);
			// resize vertex buffer appropriately
			meshlets.vertices.truncate(
				meshlets
					.meshlets
					.last()
					.map(|m| m.vertex_offset as usize + m.vertex_count as usize)
					.unwrap_or(0),
			);
			out = meshlets
		}

		let triangle_start;
		{
			profiling::scope!("meshlet indices");
			let indices = out.iter().flat_map(|m| m.triangles).copied().collect::<Vec<_>>();
			let triangles_new_len = triangle_indices_write_capacity(indices.len());
			let triangle_len = self.mesh.triangles.len();
			triangle_start = triangle_len * INDICES_PER_WORD / 3;
			self.mesh
				.triangles
				.resize(triangle_len + triangles_new_len, CompressedIndices::default());
			triangle_indices_write(
				indices.into_iter().map(u32::from),
				&mut self.mesh.triangles[triangle_len..],
			);
		}

		let draw_vertices_start;
		{
			profiling::scope!("meshlet draw vertices");
			draw_vertices_start = self.mesh.draw_vertices.len();
			self.mesh
				.draw_vertices
				.extend(out.vertices.into_iter().map(|i| s_vertices[i as usize]));
		}

		{
			profiling::scope!("meshlet data");
			let mut triangle_start = triangle_start;
			self.mesh.meshlets.extend(out.meshlets.into_iter().map(|m| {
				let data = MeshletData {
					draw_vertex_offset: MeshletOffset::new(
						draw_vertices_start + m.vertex_offset as usize,
						m.vertex_count as usize,
					),
					triangle_offset: MeshletOffset::new(triangle_start, m.triangle_count as usize),
				};
				triangle_start += m.triangle_count as usize;
				data
			}));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::gltf::Gltf;
	use crate::meshlet::lod_tree_gen::dist::Dist;
	use crate::meshlet::process::process_meshlets;
	use std::path::Path;

	const LANTERN_GLTF_PATH: &str = concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../models/models/Lantern/glTF/Lantern.gltf"
	);

	#[test]
	fn test_lantern_gltf() -> anyhow::Result<()> {
		let gltf = Gltf::open(Path::new(LANTERN_GLTF_PATH))?;
		let mut scene = process_meshlets(&gltf)?;
		for mesh in &mut scene.meshes {
			let tracker = BorderTracker::from_meshlet_mesh(mesh);
			// println!("xadj {:#?}", tracker.xadj);
			// println!("adjncy {:#?}", tracker.adjncy);
			// println!("adjncy_border_index {:#?}", tracker.adjncy_border_index);
			// println!("Borders {:#?}", tracker.borders)
			let part = tracker.metis_partition();
			println!("Partitions dist {:#?}", Dist::new(part.iter()));
		}
		Ok(())
	}
}
