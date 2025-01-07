use crate::meshlet::lod_tree_gen::indices::{IndexPair, MeshletId};
use crate::meshlet::lod_tree_gen::sorted_smallvec::SortedSmallVec;
use crate::meshlet::process::lod_mesh_build_meshlets;
use glam::FloatExt;
use meshopt::{SimplifyOptions, VertexDataAdapter};
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset_disk::material::pbr::PbrVertex;
use space_asset_disk::meshlet::lod_mesh::LodMesh;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::MESHLET_MAX_TRIANGLES;
use space_asset_disk::shape::sphere::Sphere;
use static_assertions::const_assert_eq;
use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::{offset_of, size_of, size_of_val};

#[derive(Debug)]
pub struct BorderTracker<'a> {
	/// mesh
	mesh: &'a mut LodMesh,
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
	pub fn from_meshlet_mesh(mesh: &'a mut LodMesh) -> Self {
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
			xadj = vec![0; mesh.meshlets.len() + 1];
			adjncy = Vec::with_capacity(meshlet_adj.iter().map(|s| s.len()).sum());
			for i in 0..mesh.meshlets.len() {
				xadj[i] = adjncy.len() as i32;
				adjncy.extend(meshlet_adj[i].iter().map(|id| id.0 as i32))
			}
			xadj[mesh.meshlets.len()] = adjncy.len() as i32;
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
	pub fn simplify(&mut self, lod_faction: f32, pbr_material_vertices: &[PbrVertex]) -> LodMesh {
		let groups = self.metis_partition();
		let (mesh, parent_data) = groups
			.par_iter()
			.filter_map(|group| {
				let (mesh, sphere, error) = self.simplify_meshlet_group(lod_faction, pbr_material_vertices, &group)?;
				Some((mesh, (sphere, error)))
			})
			.unzip::<_, _, LodMesh, Vec<_>>();

		for (meshlet_ids, (sphere, error)) in groups.iter().zip(parent_data.iter().copied()) {
			for meshlet_id in meshlet_ids {
				let data = &mut self.mesh.meshlets[meshlet_id.0 as usize];
				data.parent_bounds = sphere;
				data.parent_error = error;
			}
		}

		mesh
	}

	#[profiling::function]
	fn metis_partition(&self) -> Vec<SmallVec<[MeshletId; 6]>> {
		let meshlet_merge_cnt = 4;
		let n_partitions = (self.meshlets() + meshlet_merge_cnt - 1) / meshlet_merge_cnt;
		if n_partitions <= 1 {
			return Vec::from([(0..self.meshlets()).map(|id| MeshletId(id as u32)).collect()]);
		}

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
			partitions = vec![0; self.meshlets()];
			metis::Graph::new(1, n_partitions as i32, self.xadj(), self.adjncy())
				.unwrap()
				.set_adjwgt(&weights)
				.part_kway(&mut partitions)
				.unwrap();
		}

		let mut groups;
		{
			profiling::scope!("meshlet groups");
			groups = vec![SmallVec::new(); n_partitions];
			for meshlet_id in 0..self.meshlets() {
				groups[partitions[meshlet_id] as usize].push(MeshletId(meshlet_id as u32));
			}
		}

		groups
	}

	#[profiling::function]
	fn simplify_meshlet_group(
		&self,
		lod_faction: f32,
		pbr_material_vertices: &[PbrVertex],
		meshlet_ids: &[MeshletId],
	) -> Option<(LodMesh, Sphere, f32)> {
		let meshlets = meshlet_ids
			.iter()
			.map(|i| self.mesh.meshlet(i.0 as usize))
			.collect::<SmallVec<[_; 6]>>();

		let mut s_vertices;
		let mut s_indices;
		let mut s_remap;
		{
			profiling::scope!("simplify make mesh");
			let draw_vertex_cnt = meshlets.iter().map(|m| m.draw_vertex_offset.len()).sum();
			let triangle_cnt: usize = meshlets.iter().map(|m| m.triangle_offset.len()).sum();
			if draw_vertex_cnt == 0 || triangle_cnt == 0 {
				return None;
			}
			s_remap = HashMap::with_capacity(draw_vertex_cnt);
			s_vertices = Vec::with_capacity(draw_vertex_cnt);
			s_indices = Vec::with_capacity(triangle_cnt * 3);
			for m in &meshlets {
				for tri in 0..m.triangle_offset.len() {
					for i in m.load_triangle(tri).to_array() {
						let draw = m.load_draw_vertex(i as usize);
						s_indices.push(*s_remap.entry(draw.material_vertex_id).or_insert_with(|| {
							s_vertices.push(draw);
							s_vertices.len() as u32 - 1
						}));
					}
				}
			}
		}

		let s_vertex_lock = vec![false; s_vertices.len()];

		let vertex_attrib_scale = 1.;
		let vertex_attrib_weights = [
			1., 1., 1., // normal
			1., 1., // tex coord
		]
		.map(|a| a * vertex_attrib_scale);

		let s_vertex_attrib;
		{
			profiling::scope!("simplify vertex_attrib");
			s_vertex_attrib = s_vertices
				.iter()
				.flat_map(|d| {
					let pbr = pbr_material_vertices[d.material_vertex_id.0 as usize];
					[
						pbr.normal.x,
						pbr.normal.y,
						pbr.normal.z,
						pbr.tex_coord.x,
						pbr.tex_coord.y,
					]
				})
				.collect::<Vec<f32>>();
		}

		let target_count = s_indices.len() / 2;
		let target_error = f32::lerp(0.01, 0.9, lod_faction);

		let adapter = VertexDataAdapter::new(
			bytemuck::cast_slice::<DrawVertex, u8>(&s_vertices),
			size_of::<DrawVertex>(),
			offset_of!(DrawVertex, position),
		)
		.unwrap();

		let mut relative_error = 0.;
		{
			profiling::scope!("meshopt::simplify_with_attributes_and_locks");
			s_indices = meshopt::simplify_with_attributes_and_locks(
				&s_indices,
				&adapter,
				&s_vertex_attrib,
				&vertex_attrib_weights,
				size_of_val(&vertex_attrib_weights),
				&s_vertex_lock,
				target_count,
				target_error,
				SimplifyOptions::LockBorder,
				Some(&mut relative_error),
			);
		}

		let mut mesh_space_error = {
			profiling::scope!("meshopt::simplify_scale");
			// relative -> absolute error
			relative_error * meshopt::simplify_scale(&adapter)
		};
		let max_child_error = meshlets.iter().map(|m| m.error).max_by(|a, b| a.total_cmp(&b)).unwrap();
		mesh_space_error += max_child_error;

		let group_sphere =
			Sphere::merge_spheres_approx(&meshlets.iter().map(|m| m.bounds).collect::<SmallVec<[_; 6]>>()).unwrap();

		if s_indices.len() > 0 {
			Some((
				lod_mesh_build_meshlets(s_indices, s_vertices, Some(group_sphere), mesh_space_error),
				group_sphere,
				mesh_space_error,
			))
		} else {
			None
		}
	}
}
