use crate::material::pbr::PbrMaterial;
use crate::material::pbr::PbrVertex;
use crate::meshlet::indices::{CompressedIndices, triangle_indices_load};
use crate::meshlet::vertex::{DrawVertex, MaterialVertexId};
use core::ops::Deref;
use glam::UVec3;
use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{AliveDescRef, Buffer, Desc, DescRef, Descriptors};

/// not DescStruct as this should never be read or written, only constructed when querying meshlets
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MeshletReader<'a, R: DescRef> {
	pub data: MeshletData,
	pub mesh: &'a MeshletMesh<R>,
}

impl<'a, R: DescRef> Deref for MeshletReader<'a, R> {
	type Target = MeshletData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<'a, R: DescRef, T> AsRef<T> for MeshletReader<'a, R>
where
	T: ?Sized,
	<MeshletReader<'a, R> as Deref>::Target: AsRef<T>,
{
	fn as_ref(&self) -> &T {
		self.deref().as_ref()
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct MeshletMesh<R: DescRef> {
	pub meshlets: Desc<R, Buffer<[MeshletData]>>,
	pub draw_vertices: Desc<R, Buffer<[DrawVertex]>>,
	pub triangles: Desc<R, Buffer<[CompressedIndices]>>,
	pub num_meshlets: u32,
	pub pbr_material: PbrMaterial<R>,
	pub pbr_material_vertices: Desc<R, Buffer<[PbrVertex]>>,
}

impl<R: AliveDescRef> MeshletMesh<R> {
	pub fn meshlet(&self, descriptors: &Descriptors, index: usize) -> MeshletReader<'_, R> {
		assert!(
			index < self.num_meshlets as usize,
			"meshlet index out of bounds: the len is {} but the index is {}",
			self.num_meshlets as usize,
			index
		);
		MeshletReader {
			data: self.meshlets.access(descriptors).load(index),
			mesh: self,
		}
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn meshlet_unchecked(&self, descriptors: &Descriptors, index: usize) -> MeshletReader<'_, R> {
		MeshletReader {
			data: unsafe { self.meshlets.access(descriptors).load_unchecked(index) },
			mesh: self,
		}
	}
}

impl<'a, R: DescRef> MeshletReader<'a, R> {
	pub fn vertices(&self) -> usize {
		self.data.draw_vertex_offset.len()
	}

	pub fn triangles(&self) -> usize {
		self.data.triangle_offset.len()
	}
}

impl<'a, R: AliveDescRef> MeshletReader<'a, R> {
	pub fn load_draw_vertex(&self, descriptors: &Descriptors, index: usize) -> DrawVertex {
		let len = self.data.draw_vertex_offset.len();
		assert!(
			index < len,
			"index out of bounds: the len is {len} but the index is {index}"
		);
		let global_index = self.data.draw_vertex_offset.start() + index;
		self.mesh.draw_vertices.access(descriptors).load(global_index)
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn load_draw_vertex_unchecked(&self, descriptors: &Descriptors, index: usize) -> DrawVertex {
		unsafe {
			let global_index = self.data.draw_vertex_offset.start() + index;
			self.mesh.draw_vertices.access(descriptors).load_unchecked(global_index)
		}
	}

	pub fn load_triangle(&self, descriptors: &'a Descriptors, triangle: usize) -> UVec3 {
		let len = self.data.triangle_offset.len();
		assert!(
			triangle < len,
			"index out of bounds: the len is {len} but the index is {triangle}"
		);
		let triangle_indices = self.mesh.triangles.access(descriptors);
		triangle_indices_load(self, &triangle_indices, triangle, |triangle_indices, i| {
			triangle_indices.load(i)
		})
	}

	/// # Safety
	/// triangle must be in bounds
	pub unsafe fn load_triangle_unchecked(&self, descriptors: &'a Descriptors, triangle: usize) -> UVec3 {
		unsafe {
			let triangle_indices = self.mesh.triangles.access(descriptors);
			triangle_indices_load(self, &triangle_indices, triangle, |triangle_indices, i| {
				triangle_indices.load_unchecked(i)
			})
		}
	}

	pub fn load_pbr_material_vertex(&self, descriptors: &Descriptors, index: MaterialVertexId) -> PbrVertex {
		self.mesh
			.pbr_material_vertices
			.access(descriptors)
			.load(index.0 as usize)
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn load_pbr_material_vertex_unchecked(
		&self,
		descriptors: &Descriptors,
		index: MaterialVertexId,
	) -> PbrVertex {
		unsafe {
			self.mesh
				.pbr_material_vertices
				.access(descriptors)
				.load_unchecked(index.0 as usize)
		}
	}
}

pub use space_asset_disk_shader::meshlet::mesh::*;
