use crate::meshlet::offset::MeshletOffset;
use crate::meshlet::MESHLET_INDICES_BITS;
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
#[repr(transparent)]
pub struct MeshletCompactedIndex(pub u32);

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
	pub indices: StrongDesc<Buffer<[MeshletCompactedIndex]>>,
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
		self.load_vertex_unchecked(descriptors, meshlet_model, index)
	}

	#[inline]
	pub fn load_vertex_unchecked(
		&self,
		descriptors: &Descriptors,
		meshlet_model: MeshletModel,
		index: usize,
	) -> MeshletVertex {
		let global_index = self.vertex_offset.start() + index;
		meshlet_model.vertices.access(descriptors).load(global_index)
	}

	pub fn triangles(&self) -> usize {
		self.index_offset.len()
	}
}

const INDICES_PER_WORD: usize = 32 / MESHLET_INDICES_BITS as usize;
const INDICES_MASK: u32 = (1 << MESHLET_INDICES_BITS) - 1;
impl Meshlet {
	pub fn load_triangle_indices(
		&self,
		descriptors: &Descriptors,
		meshlet_model: MeshletModel,
		triangle: usize,
	) -> [u32; 3] {
		let len = self.index_offset.len();
		assert!(
			triangle < len,
			"index out of bounds: the len is {len} but the index is {triangle}"
		);
		self.load_triangle_indices_unchecked(descriptors, meshlet_model, triangle)
	}

	pub fn load_triangle_indices_unchecked(
		&self,
		descriptors: &Descriptors,
		meshlet_model: MeshletModel,
		triangle: usize,
	) -> [u32; 3] {
		let indices = meshlet_model.indices.access(descriptors);

		let mut index = (triangle * 3) / INDICES_PER_WORD;
		let mut rem = (triangle * 3) % INDICES_PER_WORD;
		let mut load = indices.load(self.index_offset.start() + index);
		let load0 = (rem, load);
		let mut load_next = || {
			rem += 1;
			if rem == INDICES_PER_WORD {
				rem = 0;
				index += 1;
				load = indices.load(self.index_offset.start() + index);
			}
			(rem, load)
		};
		let loads = [load0, load_next(), load_next()];

		loads.map(|(rem, load)| (load.0 >> (rem as u32 * MESHLET_INDICES_BITS)) & INDICES_MASK)
	}
}
