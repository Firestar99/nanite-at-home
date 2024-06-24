mod gpu {
	use crate::meshlet::indices::{triangle_indices_load, CompressedIndices};
	use crate::meshlet::offset::MeshletOffset;
	use core::mem;
	use core::ops::Deref;
	use glam::UVec3;
	use static_assertions::const_assert_eq;
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::reference::{AliveDescRef, Desc, DescRef};
	use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors};

	use crate::meshlet::vertex::MeshletVertex;

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
	pub struct MeshletData {
		pub vertex_offset: MeshletOffset,
		pub triangle_indices_offset: MeshletOffset,
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
		pub vertices: Desc<R, Buffer<[MeshletVertex]>>,
		pub triangle_indices: Desc<R, Buffer<[CompressedIndices]>>,
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
			self.data.vertex_offset.len()
		}

		pub fn triangles(&self) -> usize {
			self.data.triangle_indices_offset.len()
		}
	}

	impl<'a, R: AliveDescRef> Meshlet<'a, R> {
		pub fn load_vertex(&self, descriptors: &Descriptors, index: usize) -> MeshletVertex {
			let len = self.data.vertex_offset.len();
			assert!(
				index < len,
				"index out of bounds: the len is {len} but the index is {index}"
			);
			unsafe { self.load_vertex_unchecked(descriptors, index) }
		}

		/// # Safety
		/// index must be in bounds
		#[inline]
		pub unsafe fn load_vertex_unchecked(&self, descriptors: &Descriptors, index: usize) -> MeshletVertex {
			let global_index = self.data.vertex_offset.start() + index;
			self.mesh.vertices.access(descriptors).load_unchecked(global_index)
		}

		#[inline]
		pub fn load_triangle_indices(&self, descriptors: &'a Descriptors, triangle: usize) -> UVec3 {
			let len = self.data.triangle_indices_offset.len();
			assert!(
				triangle < len,
				"index out of bounds: the len is {len} but the index is {triangle}"
			);
			let triangle_indices = self.mesh.triangle_indices.access(descriptors);
			triangle_indices_load(self, &triangle_indices, triangle, |triangle_indices, i| {
				triangle_indices.load(i)
			})
		}

		/// # Safety
		/// triangle must be in bounds
		#[inline]
		pub unsafe fn load_triangle_indices_unchecked(&self, descriptors: &'a Descriptors, triangle: usize) -> UVec3 {
			unsafe {
				let triangle_indices = self.mesh.triangle_indices.access(descriptors);
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
	use crate::meshlet::indices::CompressedIndices;
	use crate::meshlet::mesh::MeshletData;
	use crate::meshlet::vertex::MeshletVertex;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Archive, Serialize, Deserialize)]
	pub struct MeshletMeshDisk {
		pub meshlets: Vec<MeshletData>,
		pub vertices: Vec<MeshletVertex>,
		pub triangle_indices: Vec<CompressedIndices>,
	}
}
#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "cpu")]
mod cpu {
	use crate::meshlet::mesh2instance::MeshletMesh2Instance;
	use std::ops::Deref;
	use vulkano_bindless::descriptor::RC;
	use vulkano_bindless_shaders::descriptor::reference::Strong;

	pub struct MeshletMesh2InstanceCpu {
		pub mesh2instance: MeshletMesh2Instance<RC, Strong>,
		pub num_meshlets: u32,
	}

	impl Deref for MeshletMesh2InstanceCpu {
		type Target = MeshletMesh2Instance<RC, Strong>;

		fn deref(&self) -> &Self::Target {
			&self.mesh2instance
		}
	}
}
#[cfg(feature = "cpu")]
pub use cpu::*;
