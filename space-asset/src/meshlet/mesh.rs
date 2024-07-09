mod gpu {
	use crate::material::pbr::PbrMaterial;
	use crate::meshlet::indices::{triangle_indices_load, CompressedIndices};
	use crate::meshlet::offset::MeshletOffset;
	use crate::meshlet::vertex::{DrawVertex, EncodedDrawVertex};
	use bytemuck_derive::AnyBitPattern;
	use core::mem;
	use core::ops::Deref;
	use glam::UVec3;
	use static_assertions::const_assert_eq;
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::reference::{AliveDescRef, Desc, DescRef};
	use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors};

	#[repr(C)]
	#[derive(Copy, Clone, Debug, AnyBitPattern)]
	#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
	pub struct MeshletData {
		pub draw_vertex_offset: MeshletOffset,
		pub triangle_offset: MeshletOffset,
	}
	const_assert_eq!(mem::size_of::<MeshletData>(), 2 * 4);

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
		pub draw_vertices: Desc<R, Buffer<[EncodedDrawVertex]>>,
		pub triangles: Desc<R, Buffer<[CompressedIndices]>>,
		pub pbr_material: PbrMaterial<R>,
		pub num_meshlets: u32,
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
			self.mesh.draw_vertices.access(descriptors).load(global_index).decode()
		}

		/// # Safety
		/// index must be in bounds
		pub unsafe fn load_draw_vertex_unchecked(&self, descriptors: &Descriptors, index: usize) -> DrawVertex {
			unsafe {
				let global_index = self.data.draw_vertex_offset.start() + index;
				self.mesh
					.draw_vertices
					.access(descriptors)
					.load_unchecked(global_index)
					.decode()
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
	}
}
pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::material::pbr::PbrMaterialDisk;
	use crate::meshlet::indices::CompressedIndices;
	use crate::meshlet::mesh::MeshletData;
	use crate::meshlet::vertex::EncodedDrawVertex;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct MeshletMeshDisk {
		pub meshlets: Vec<MeshletData>,
		pub draw_vertices: Vec<EncodedDrawVertex>,
		pub triangles: Vec<CompressedIndices>,
		pub pbr_material: PbrMaterialDisk,
	}
}
#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::meshlet::mesh::{ArchivedMeshletMeshDisk, MeshletMesh};
	use crate::uploader::{deserialize_infallible, UploadError, Uploader};
	use vulkano::Validated;
	use vulkano_bindless::descriptor::{RCDescExt, RC};
	use vulkano_bindless_shaders::descriptor::reference::Strong;

	impl MeshletMesh<RC> {
		pub fn to_strong(&self) -> MeshletMesh<Strong> {
			MeshletMesh {
				meshlets: self.meshlets.to_strong(),
				draw_vertices: self.draw_vertices.to_strong(),
				triangles: self.triangles.to_strong(),
				pbr_material: self.pbr_material.to_strong(),
				num_meshlets: self.num_meshlets,
			}
		}
	}

	impl ArchivedMeshletMeshDisk {
		pub async fn upload(&self, uploader: &Uploader) -> Result<MeshletMesh<RC>, Validated<UploadError>> {
			let meshlets = uploader.upload_buffer_iter(self.meshlets.iter().map(deserialize_infallible));
			let draw_vertices = uploader.upload_buffer_iter(self.draw_vertices.iter().map(deserialize_infallible));
			let triangles = uploader.upload_buffer_iter(self.triangles.iter().map(deserialize_infallible));
			let pbr_material = self.pbr_material.upload(uploader);
			Ok(MeshletMesh {
				meshlets: meshlets.await?.into(),
				draw_vertices: draw_vertices.await?.into(),
				pbr_material: pbr_material.await?,
				triangles: triangles.await?.into(),
				num_meshlets: self.meshlets.len() as u32,
			})
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
