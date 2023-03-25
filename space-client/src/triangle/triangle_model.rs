use std::ops::Deref;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::impl_vertex;
use vulkano::memory::allocator::MemoryAllocator;

pub struct TriangleModel(pub Arc<CpuAccessibleBuffer<[Vertex]>>);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct Vertex {
	position: [f32; 2],
}

impl_vertex!(Vertex, position);

impl TriangleModel {
	pub fn new_basic_model(allocator: &(impl MemoryAllocator + ?Sized)) -> TriangleModel {
		let vertices = [
			Vertex {
				position: [-0.5, -0.25],
			},
			Vertex {
				position: [0.0, 0.5],
			},
			Vertex {
				position: [0.25, -0.1],
			},
		];

		let buffer = CpuAccessibleBuffer::from_iter(
			allocator,
			BufferUsage {
				vertex_buffer: true,
				..BufferUsage::empty()
			},
			false,
			vertices,
		).unwrap();

		TriangleModel(buffer)
	}
}

impl Deref for TriangleModel {
	type Target = Arc<CpuAccessibleBuffer<[Vertex]>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
