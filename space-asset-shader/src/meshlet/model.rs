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
pub struct Meshlet {
	pub vertex_offset: MeshletOffset,
	pub index_offset: MeshletOffset,
}
const_assert_eq!(mem::size_of::<Meshlet>(), 2 * 4);

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletModel {
	pub meshlets: StrongDesc<Buffer<[Meshlet]>>,
	pub vertices: StrongDesc<Buffer<[MeshletVertex]>>,
	pub indices: StrongDesc<Buffer<[CompressedIndices]>>,
}

impl Meshlet {
	pub fn vertices(&self) -> usize {
		self.vertex_offset.len()
	}

	pub fn load_vertex(&self, descriptors: &Descriptors, meshlet_model: MeshletModel, index: usize) -> MeshletVertex {
		let len = self.vertex_offset.len();
		assert!(
			index < len,
			"index out of bounds: the len is {len} but the index is {index}"
		);
		unsafe { self.load_vertex_unchecked(descriptors, meshlet_model, index) }
	}

	/// # Safety
	/// must be in bounds
	#[inline]
	pub unsafe fn load_vertex_unchecked(
		&self,
		descriptors: &Descriptors,
		meshlet_model: MeshletModel,
		index: usize,
	) -> MeshletVertex {
		let global_index = self.vertex_offset.start() + index;
		meshlet_model.vertices.access(descriptors).load_unchecked(global_index)
	}

	pub fn triangles(&self) -> usize {
		self.index_offset.len()
	}

	pub fn indices_reader<'a>(
		&self,
		descriptors: &'a Descriptors,
		meshlet_model: MeshletModel,
	) -> IndicesReader<SourceGpu<'a>> {
		IndicesReader::from_bindless(descriptors, meshlet_model, *self)
	}
}
