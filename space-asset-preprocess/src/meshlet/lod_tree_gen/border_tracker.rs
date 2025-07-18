use crate::meshlet::lod_mesh::LodMesh;
use crate::meshlet::lod_tree_gen::indices::{IndexPair, MeshletId};
use crate::meshlet::lod_tree_gen::sorted_smallvec::SortedSmallVec;
use crate::meshlet::mesh::MeshletMesh;
use crate::meshlet::process::lod_mesh_build_meshlets;
use MeshletGroupSimplifyResult::*;
use glam::FloatExt;
use meshopt::{SimplifyOptions, VertexDataAdapter};
use rayon::prelude::*;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::SmallVec;
use space_asset_disk::material::pbr::PbrVertex;
use space_asset_disk::meshlet::lod_level_bitmask::LodLevelBitmask;
use space_asset_disk::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset_disk::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use space_asset_disk::shape::sphere::Sphere;
use static_assertions::const_assert_eq;
use std::fmt::Debug;
use std::mem::{offset_of, size_of, size_of_val};
use std::ops::Deref;

const MAX_LOD_LEVEL: u32 = 50;
const TARGET_SIMPLIFICATION_FACTOR: f32 = 0.5;
const MINIMUM_REQUIRED_SIMPLIFICATION_FACTOR: f32 = 0.65;

pub fn process_lod_tree(mut mesh: MeshletMesh) -> anyhow::Result<MeshletMesh> {
	for m in &mut mesh.lod_mesh.meshlets {
		m.lod_level_bitmask = LodLevelBitmask(1);
	}
	let mut queue = (0..mesh.lod_mesh.meshlets.len() as u32)
		.map(MeshletId)
		.collect::<Vec<_>>();
	let mut prev_groups = None;

	let mut lod_levels = 1..MAX_LOD_LEVEL;
	for lod_level in &mut lod_levels {
		let lod_faction = lod_level as f32 / MAX_LOD_LEVEL as f32;

		// If prev_groups exists, tracker also doesn't get recreated thanks to the optimizer, according to profiling.
		// Manually reusing it is hard due to its lifetime, so this has to be sufficient for now.
		let tracker = BorderTracker::from_meshlet_mesh(&mesh.lod_mesh, &queue);
		let groups = prev_groups.take().unwrap_or_else(|| tracker.metis_partition());
		assert!(!groups.is_empty());

		let (mut lod, parent_data) = groups
			.par_iter()
			.map(
				|group| match tracker.simplify_meshlet_group(group, &mesh.pbr_material_vertices, lod_faction) {
					SimplifiedMeshlets(mesh, sphere, error) => {
						(mesh, SimplifiedMeshlets(LodMesh::default(), sphere, error))
					}
					TooLittleSimplification => (LodMesh::default(), TooLittleSimplification),
					SimplifiedToNothing => (LodMesh::default(), SimplifiedToNothing),
				},
			)
			.unzip::<_, _, LodMesh, Vec<_>>();

		if parent_data.iter().all(|a| matches!(a, SimplifiedToNothing)) {
			// must break before clearing queue
			break;
		}
		if parent_data.iter().all(|a| matches!(a, TooLittleSimplification)) {
			// retry with same groups, don't rerun METIS
			prev_groups = Some(groups);
			continue;
		}

		let groups = tracker.groups_to_meshlet_ids(&groups);
		drop(tracker);
		queue.clear();

		let lod_mask = 1u32.checked_shl(lod_level).map(LodLevelBitmask);
		for (meshlet_ids, result) in groups.iter().zip(parent_data.iter()) {
			match result {
				SimplifiedMeshlets(_, sphere, error) => {
					for meshlet_id in meshlet_ids {
						let data = &mut mesh.meshlets[meshlet_id.0 as usize];
						data.parent_bounds = *sphere;
						data.parent_error = *error;
					}
				}
				TooLittleSimplification => {
					if let Some(lod_mask) = lod_mask {
						for id in meshlet_ids {
							mesh.lod_mesh.meshlet_mut(*id).lod_level_bitmask |= lod_mask;
						}
					}
					queue.extend_from_slice(meshlet_ids);
				}
				SimplifiedToNothing => (),
			}
		}

		if let Some(lod_mask) = lod_mask {
			for m in &mut lod.meshlets {
				m.lod_level_bitmask |= lod_mask;
			}
		}

		queue.extend((0..lod.meshlets.len()).map(|a| MeshletId((a + mesh.lod_mesh.meshlets.len()) as u32)));
		mesh.lod_mesh.append(&mut lod);
	}

	if let Some(lod_mask) = (!0u32).checked_shl(lod_levels.start) {
		for meshlet_id in queue {
			mesh.meshlets[meshlet_id.0 as usize].lod_level_bitmask |= LodLevelBitmask(lod_mask);
		}
	}
	Ok(mesh)
}

#[derive(Debug)]
pub struct BorderTracker<'a> {
	/// mesh
	mesh: &'a LodMesh,
	/// meshlet ids to be simplified
	queued_meshlets: &'a [MeshletId],
	/// Compressed Sparse Row `xadj` like METIS
	xadj: Vec<i32>,
	/// Compressed Sparse Row `adjncy` like METIS, adjacency indices are sorted
	adjncy: Vec<i32>,
	/// Compressed Sparse Row `adjncy` like METIS,
	/// but instead of containing adjacency indices it contains indices into `borders`
	adjncy_border_index: Vec<u32>,
	/// contains all borders
	borders: Vec<Border>,
	/// remap table to deduplicate material vertices based on their position
	position_dedup_material_remap: FxHashMap<[u32; 3], MaterialVertexId>,
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

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QueueId(pub u32);

impl Deref for QueueId {
	type Target = u32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'a> BorderTracker<'a> {
	pub fn get_connected_meshlets(&self, meshlet: QueueId) -> impl Iterator<Item = QueueId> + '_ {
		let adjncy_range = self.xadj[*meshlet as usize] as usize..self.xadj[*meshlet as usize + 1] as usize;
		self.adjncy[adjncy_range].iter().map(|i| QueueId(*i as u32))
	}

	pub fn get_border(&self, meshlets: IndexPair<QueueId>) -> Option<&Border> {
		let adjncy_range = self.xadj[*meshlets.0 as usize] as usize..self.xadj[*meshlets.0 as usize + 1] as usize;
		let adjncy_index = adjncy_range.start + self.adjncy[adjncy_range].binary_search(&(*meshlets.1 as i32)).ok()?;
		Some(&self.borders[self.adjncy_border_index[adjncy_index] as usize])
	}

	pub fn get_position_dedup_material_vertex_id(&self, vertex: DrawVertex) -> Option<MaterialVertexId> {
		self.position_dedup_material_remap
			.get(&vertex.position.to_array().map(|f| f.to_bits()))
			.copied()
	}

	pub fn xadj(&self) -> &[i32] {
		&self.xadj
	}

	pub fn adjncy(&self) -> &[i32] {
		&self.adjncy
	}

	pub fn queued_meshlets(&self) -> usize {
		self.xadj.len() - 1
	}

	pub fn from_meshlet_mesh(mesh: &'a LodMesh, queued_meshlets: &'a [MeshletId]) -> Self {
		profiling::function_scope!();
		// SmallVec: most Edges have only 1 meshlet, some 2, and in extremely rare cases >2
		// But we get a capacity of 4 for free, as SmallVec's heap alloc needs 16 bytes anyway
		const_assert_eq!(
			size_of::<SmallVec<[MeshletId; 1]>>(),
			size_of::<SmallVec<[MeshletId; 4]>>()
		);
		let mut edge_to_meshlets: FxHashMap<IndexPair<MaterialVertexId>, SmallVec<[QueueId; 4]>>;
		let mut position_dedup_material_remap: FxHashMap<[u32; 3], MaterialVertexId>;
		// Use a SmallVec of cap 6 for adjacency:
		// small models: 0..2
		// large models: usually 0..6, sometimes a few meshlets have significantly more
		let mut meshlet_adj: Vec<SortedSmallVec<[QueueId; 6]>>;
		{
			profiling::scope!("edge_to_meshlets meshlet_adj");
			// HashMap capacity: worst case we get 2 new edges per triangle, but around 1 edge per triangle is typical
			// and HashMap over allocates a bit anyway
			edge_to_meshlets = FxHashMap::with_capacity_and_hasher(
				queued_meshlets.len() * MESHLET_MAX_TRIANGLES as usize,
				FxBuildHasher,
			);
			position_dedup_material_remap = FxHashMap::with_capacity_and_hasher(
				queued_meshlets.len() * MESHLET_MAX_VERTICES as usize,
				FxBuildHasher,
			);
			meshlet_adj = vec![SortedSmallVec::new(); queued_meshlets.len()];
			for queue_id in 0..queued_meshlets.len() {
				let queue_id = QueueId(queue_id as u32);
				let meshlet_id = queued_meshlets[queue_id.0 as usize];
				let meshlet = mesh.meshlet(meshlet_id);
				for triangle in 0..meshlet.triangle_offset.len() {
					let draw_indices = meshlet.load_triangle(triangle);
					let indices = draw_indices.to_array().map(|i| {
						let vertex = meshlet.load_draw_vertex(i as usize);
						*position_dedup_material_remap
							.entry(vertex.position.to_array().map(|f| f.to_bits()))
							.or_insert(vertex.material_vertex_id)
					});
					let edges = (0..3).map(|i| IndexPair::new(indices[i], indices[(i + 1) % 3]));
					for edge in edges {
						// it's not worth optimizing this search for (typically) at most 2 entries, just do a linear scan
						let vec = edge_to_meshlets.entry(edge).or_default();
						if !vec.is_empty() {
							if vec.contains(&queue_id) {
								continue;
							}
							let x = &mut meshlet_adj[queue_id.0 as usize];
							for other_meshlet_id in vec.iter().copied() {
								x.insert(other_meshlet_id);
							}
							for other_meshlet_id in vec.iter().copied() {
								meshlet_adj[*other_meshlet_id as usize].insert(queue_id);
							}
						}
						vec.push(queue_id);
					}
				}
			}
		}

		let mut xadj;
		let mut adjncy;
		{
			profiling::scope!("xadj adjncy");
			xadj = vec![0; queued_meshlets.len() + 1];
			adjncy = Vec::with_capacity(meshlet_adj.iter().map(|s| s.len()).sum());
			for i in 0..queued_meshlets.len() {
				xadj[i] = adjncy.len() as i32;
				adjncy.extend(meshlet_adj[i].iter().map(|id| id.0 as i32))
			}
			xadj[queued_meshlets.len()] = adjncy.len() as i32;
			assert_eq!(xadj.len(), queued_meshlets.len() + 1);
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
			queued_meshlets,
			xadj,
			adjncy,
			adjncy_border_index,
			borders,
			position_dedup_material_remap,
		}
	}

	#[allow(clippy::needless_range_loop)]
	pub fn metis_partition(&self) -> Vec<SmallVec<[QueueId; 6]>> {
		profiling::function_scope!();
		let meshlet_merge_cnt = 4;
		let n_partitions = self.queued_meshlets().div_ceil(meshlet_merge_cnt);
		if n_partitions <= 1 {
			return Vec::from([(0..self.queued_meshlets.len()).map(|i| QueueId(i as u32)).collect()]);
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
			partitions = vec![0; self.queued_meshlets()];
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
			for queue_id in 0..self.queued_meshlets() {
				groups[partitions[queue_id] as usize].push(QueueId(queue_id as u32));
			}
		}

		groups
	}

	pub fn groups_to_meshlet_ids(&self, groups: &[SmallVec<[QueueId; 6]>]) -> Vec<SmallVec<[MeshletId; 6]>> {
		groups
			.iter()
			.map(|group| group.iter().map(|i| self.queued_meshlets[i.0 as usize]).collect())
			.collect()
	}

	pub fn simplify_meshlet_group(
		&self,
		queue_ids: &[QueueId],
		pbr_material_vertices: &[PbrVertex],
		lod_faction: f32,
	) -> MeshletGroupSimplifyResult {
		profiling::function_scope!();
		let meshlets = queue_ids
			.iter()
			.map(|i| self.mesh.meshlet(self.queued_meshlets[i.0 as usize]))
			.collect::<SmallVec<[_; 6]>>();

		let draw_vertex_cnt = meshlets.iter().map(|m| m.draw_vertex_offset.len()).sum();
		let triangle_cnt: usize = meshlets.iter().map(|m| m.triangle_offset.len()).sum();
		if draw_vertex_cnt == 0 || triangle_cnt == 0 {
			return SimplifiedToNothing;
		}

		let mut s_vertices;
		let mut s_indices;
		{
			profiling::scope!("simplify make mesh");
			let mut s_remap = FxHashMap::with_capacity_and_hasher(draw_vertex_cnt, FxBuildHasher);
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

		let s_vertex_lock;
		{
			profiling::scope!("simplify vertex_lock");
			let mut locked_dedup_material_ids = FxHashSet::with_capacity_and_hasher(draw_vertex_cnt, FxBuildHasher);
			for id in queue_ids.iter().copied() {
				for oid in self.get_connected_meshlets(id) {
					if !queue_ids.contains(&oid) {
						for edge in &self.get_border(IndexPair::new(id, oid)).unwrap().edges {
							for vtx_id in edge.iter() {
								locked_dedup_material_ids.insert(vtx_id);
							}
						}
					}
				}
			}

			s_vertex_lock = s_vertices
				.iter()
				.map(|v| {
					locked_dedup_material_ids.contains(
						&self
							.get_position_dedup_material_vertex_id(*v)
							.expect("DrawVertex without position deduplicated material vertex id"),
					)
				})
				.collect::<Vec<_>>();
		}

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

		let original_indices_cnt = s_indices.len();
		let target_count = (original_indices_cnt as f32 * TARGET_SIMPLIFICATION_FACTOR) as usize;
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
				SimplifyOptions::empty(),
				Some(&mut relative_error),
			);
		}

		let simplification_factor = s_indices.len() as f32 / original_indices_cnt as f32;
		if simplification_factor > MINIMUM_REQUIRED_SIMPLIFICATION_FACTOR {
			return TooLittleSimplification;
		}

		let mut mesh_space_error = {
			profiling::scope!("meshopt::simplify_scale");
			// relative -> absolute error
			relative_error * meshopt::simplify_scale(&adapter)
		};
		let max_child_error = meshlets.iter().map(|m| m.error).max_by(|a, b| a.total_cmp(b)).unwrap();
		mesh_space_error += max_child_error;

		let group_sphere =
			Sphere::merge_spheres_approx(&meshlets.iter().map(|m| m.bounds).collect::<SmallVec<[_; 6]>>()).unwrap();

		if !s_indices.is_empty() {
			SimplifiedMeshlets(
				lod_mesh_build_meshlets(&mut s_indices, &mut s_vertices, Some(group_sphere), mesh_space_error),
				group_sphere,
				mesh_space_error,
			)
		} else {
			SimplifiedToNothing
		}
	}
}

pub enum MeshletGroupSimplifyResult {
	TooLittleSimplification,
	SimplifiedMeshlets(LodMesh, Sphere, f32),
	SimplifiedToNothing,
}
