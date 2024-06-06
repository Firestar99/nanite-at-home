use crate::meshlet::indices::{CompressedIndices, IndicesReader, SourceGpu};
use crate::meshlet::offset::MeshletOffset;
use core::mem;
use glam::Vec3;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::{Buffer, Descriptors, ValidDesc};

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletVertex {
	pub position: [f32; 3],
}
const_assert_eq!(mem::size_of::<MeshletVertex>(), 3 * 4);

impl MeshletVertex {
	pub fn position(&self) -> Vec3 {
		Vec3::from(self.position)
	}
}

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletData {
	pub vertex_offset: MeshletOffset,
	pub index_offset: MeshletOffset,
}
const_assert_eq!(mem::size_of::<MeshletData>(), 2 * 4);

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Meshlet {
	pub data: MeshletData,
	pub mesh: MeshletMesh,
}

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletMesh {
	pub meshlets: StrongDesc<Buffer<[MeshletData]>>,
	pub vertices: StrongDesc<Buffer<[MeshletVertex]>>,
	pub indices: StrongDesc<Buffer<[CompressedIndices]>>,
	pub num_meshlets: u32,
}

impl MeshletMesh {
	pub fn meshlet(&self, descriptors: &Descriptors, index: usize) -> Meshlet {
		assert!(
			index < self.num_meshlets as usize,
			"meshlet index out of bounds: the len is {} but the index is {}",
			self.num_meshlets as usize,
			index
		);
		Meshlet {
			data: self.meshlets.access(descriptors).load(index),
			mesh: *self,
		}
	}

	/// # Safety
	/// index must be in bounds
	pub unsafe fn meshlet_unchecked(&self, descriptors: &Descriptors, index: usize) -> Meshlet {
		Meshlet {
			data: unsafe { self.meshlets.access(descriptors).load_unchecked(index) },
			mesh: *self,
		}
	}
}

impl Meshlet {
	pub fn vertices(&self) -> usize {
		self.data.vertex_offset.len()
	}

	pub fn load_vertex(&self, descriptors: &Descriptors, index: usize) -> MeshletVertex {
		let len = self.data.vertex_offset.len();
		assert!(
			index < len,
			"index out of bounds: the len is {len} but the index is {index}"
		);
		unsafe { self.load_vertex_unchecked(descriptors, index) }
	}

	/// # Safety
	/// must be in bounds
	#[inline]
	pub unsafe fn load_vertex_unchecked(&self, descriptors: &Descriptors, index: usize) -> MeshletVertex {
		let global_index = self.data.vertex_offset.start() + index;
		self.mesh.vertices.access(descriptors).load_unchecked(global_index)
	}

	pub fn triangles(&self) -> usize {
		self.data.index_offset.len()
	}

	pub fn indices_reader<'a>(&self, descriptors: &'a Descriptors) -> IndicesReader<SourceGpu<'a>> {
		IndicesReader::from_bindless(descriptors, *self)
	}
}
