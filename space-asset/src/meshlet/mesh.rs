mod gpu {
	use crate::material::pbr::vertex::PbrVertex;
	use crate::material::pbr::PbrMaterial;
	use crate::meshlet::indices::{triangle_indices_load, CompressedIndices};
	use crate::meshlet::offset::MeshletOffset;
	use crate::meshlet::vertex::{DrawVertex, MaterialVertexId};
	use core::ops::Deref;
	use glam::UVec3;
	use vulkano_bindless_macros::{assert_transfer_size, BufferContent};
	use vulkano_bindless_shaders::descriptor::{AliveDescRef, Buffer, Desc, DescRef, Descriptors};

	#[repr(C)]
	#[derive(Copy, Clone, Debug, BufferContent)]
	#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
	pub struct MeshletData {
		pub draw_vertex_offset: MeshletOffset,
		pub triangle_offset: MeshletOffset,
	}
	assert_transfer_size!(MeshletData, 2 * 4);

	impl AsRef<MeshletData> for MeshletData {
		fn as_ref(&self) -> &MeshletData {
			self
		}
	}

	/// not DescStruct as this should never be read or written, only constructed when querying meshlets
	#[repr(C)]
	#[derive(Copy, Clone)]
	pub struct Meshlet<'a, R: DescRef> {
		pub data: MeshletData,
		pub mesh: &'a MeshletMesh<R>,
	}

	impl<'a, R: DescRef> Deref for Meshlet<'a, R> {
		type Target = MeshletData;

		fn deref(&self) -> &Self::Target {
			&self.data
		}
	}

	impl<'a, R: DescRef, T> AsRef<T> for Meshlet<'a, R>
	where
		T: ?Sized,
		<Meshlet<'a, R> as Deref>::Target: AsRef<T>,
	{
		fn as_ref(&self) -> &T {
			self.deref().as_ref()
		}
	}

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	pub struct MeshletMesh<R: DescRef> {
		pub meshlets: Desc<R, Buffer<[MeshletData]>>,
		pub draw_vertices: Desc<R, Buffer<[DrawVertex]>>,
		pub triangles: Desc<R, Buffer<[CompressedIndices]>>,
		pub num_meshlets: u32,
		pub pbr_material: PbrMaterial<R>,
		pub pbr_material_vertices: Desc<R, Buffer<[PbrVertex]>>,
	}

	impl<R: AliveDescRef> MeshletMesh<R> {
		pub fn meshlet(&self, descriptors: &Descriptors, index: usize) -> Meshlet<R> {
			assert!(
				index < self.num_meshlets as usize,
				"meshlet index out of bounds: the len is {} but the index is {}",
				self.num_meshlets as usize,
				index
			);
			Meshlet {
				data: self.meshlets.access(descriptors).load(index),
				mesh: self,
			}
		}

		/// # Safety
		/// index must be in bounds
		pub unsafe fn meshlet_unchecked(&self, descriptors: &Descriptors, index: usize) -> Meshlet<R> {
			Meshlet {
				data: unsafe { self.meshlets.access(descriptors).load_unchecked(index) },
				mesh: self,
			}
		}
	}

	impl<'a, R: DescRef> Meshlet<'a, R> {
		pub fn vertices(&self) -> usize {
			self.data.draw_vertex_offset.len()
		}

		pub fn triangles(&self) -> usize {
			self.data.triangle_offset.len()
		}
	}

	impl<'a, R: AliveDescRef> Meshlet<'a, R> {
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
}
pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::material::pbr::vertex::PbrVertex;
	use crate::meshlet::indices::CompressedIndices;
	use crate::meshlet::mesh::MeshletData;
	use crate::meshlet::vertex::DrawVertex;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletMeshDisk {
		pub meshlets: Vec<MeshletData>,
		pub draw_vertices: Vec<DrawVertex>,
		pub triangles: Vec<CompressedIndices>,
		pub pbr_material_vertices: Vec<PbrVertex>,
		pub pbr_material_id: u32,
	}
}
#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::material::pbr::PbrMaterial;
	use crate::meshlet::mesh::{ArchivedMeshletMeshDisk, MeshletMesh};
	use crate::uploader::{deserialize_infallible, UploadError, Uploader};
	use std::future::Future;
	use vulkano::Validated;
	use vulkano_bindless::descriptor::{RCDescExt, Strong, RC};

	impl MeshletMesh<RC> {
		pub fn to_strong(&self) -> MeshletMesh<Strong> {
			MeshletMesh {
				meshlets: self.meshlets.to_strong(),
				draw_vertices: self.draw_vertices.to_strong(),
				triangles: self.triangles.to_strong(),
				num_meshlets: self.num_meshlets,
				pbr_material: self.pbr_material.to_strong(),
				pbr_material_vertices: self.pbr_material_vertices.to_strong(),
			}
		}
	}

	impl ArchivedMeshletMeshDisk {
		pub fn upload<'a>(
			&'a self,
			uploader: &'a Uploader,
			pbr_materials: &'a [PbrMaterial<RC>],
		) -> impl Future<Output = Result<MeshletMesh<RC>, Validated<UploadError>>> + 'a {
			let meshlets = uploader.upload_buffer_iter(self.meshlets.iter().map(deserialize_infallible));
			let draw_vertices = uploader.upload_buffer_iter(self.draw_vertices.iter().map(deserialize_infallible));
			let triangles = uploader.upload_buffer_iter(self.triangles.iter().map(deserialize_infallible));
			let pbr_material_vertices =
				uploader.upload_buffer_iter(self.pbr_material_vertices.iter().map(deserialize_infallible));
			async {
				Ok(MeshletMesh {
					meshlets: meshlets.await?.into(),
					draw_vertices: draw_vertices.await?.into(),
					triangles: triangles.await?.into(),
					num_meshlets: self.meshlets.len() as u32,
					pbr_material: pbr_materials.get(self.pbr_material_id as usize).unwrap().clone(),
					pbr_material_vertices: pbr_material_vertices.await?.into(),
				})
			}
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
