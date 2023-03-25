use std::f32::consts::PI;
use std::ops::{Deref, DerefMut};
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
		let buffer = CpuAccessibleBuffer::from_iter(
			allocator,
			BufferUsage {
				vertex_buffer: true,
				..BufferUsage::empty()
			},
			false,
			TriangleModel::state(0f32),
		).unwrap();

		TriangleModel(buffer)
	}

	fn state(time: f32) -> [Vertex; 3] {
		// [
		// 	Vertex {
		// 		position: [-0.5, -0.25],
		// 	},
		// 	Vertex {
		// 		position: [0.0, 0.5],
		// 	},
		// 	Vertex {
		// 		position: [0.25, -0.1],
		// 	},
		// ]

		[0., 120., 240.]
			.map(|x| (time + (x / 360.)) * 2. * PI)
			.map(|x| Vertex { position: [x.sin(), x.cos()] })
	}

	pub fn update(&self, time: f32) {
		self.0.write().unwrap().deref_mut().copy_from_slice(TriangleModel::state(time).as_slice());
	}
}

impl Deref for TriangleModel {
	type Target = Arc<CpuAccessibleBuffer<[Vertex]>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
