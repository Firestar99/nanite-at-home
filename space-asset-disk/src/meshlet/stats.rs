use glam::Vec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::iter::Sum;
use std::ops::{Add, AddAssign};

#[derive(Clone, Default, Debug, Archive, Serialize, Deserialize)]
pub struct MeshletSceneStats {
	pub source: SourceMeshStats,
}

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct SourceMeshStats {
	/// unique vertices as in the source mesh
	pub unique_vertices: u32,
	pub triangles: u32,
	pub meshlets: u32,
	/// vertices may be duplicated / processed by multiple meshlets
	pub meshlet_vertices: u32,
	pub bounds_min: Vec3,
	pub bounds_max: Vec3,
}

impl Default for SourceMeshStats {
	fn default() -> Self {
		Self {
			meshlets: 0,
			unique_vertices: 0,
			meshlet_vertices: 0,
			triangles: 0,
			bounds_min: Vec3::INFINITY,
			bounds_max: Vec3::NEG_INFINITY,
		}
	}
}

impl Add for SourceMeshStats {
	type Output = SourceMeshStats;

	fn add(self, rhs: Self) -> Self::Output {
		Self {
			meshlets: self.meshlets + rhs.meshlets,
			unique_vertices: self.unique_vertices + rhs.unique_vertices,
			meshlet_vertices: self.meshlet_vertices + rhs.meshlet_vertices,
			triangles: self.triangles + rhs.triangles,
			bounds_min: self.bounds_min.min(rhs.bounds_min),
			bounds_max: self.bounds_max.max(rhs.bounds_max),
		}
	}
}

impl AddAssign for SourceMeshStats {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sum for SourceMeshStats {
	fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
		iter.fold(Self::default(), |acc, x| acc + x)
	}
}
