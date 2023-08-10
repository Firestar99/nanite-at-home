#![cfg(not(target_arch = "spirv"))]

use std::f32::consts::PI;
use std::ops::Deref;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryUsage};
use vulkano::pipeline::graphics::vertex_input::Vertex;

pub struct TriangleModel(pub Subbuffer<[TriangleVertex]>);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod, Vertex)]
pub struct TriangleVertex {
	#[format(R32G32_SFLOAT)]
	position: [f32; 2],
}

impl TriangleModel {
	pub fn new_basic_model(allocator: &(impl MemoryAllocator + ?Sized)) -> TriangleModel {
		let buffer = Buffer::from_iter(
			allocator,
			BufferCreateInfo {
				usage: BufferUsage::VERTEX_BUFFER,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				usage: MemoryUsage::Upload,
				..Default::default()
			},
			TriangleModel::state(0f32),
		).unwrap();

		TriangleModel(buffer)
	}

	fn state(time: f32) -> [TriangleVertex; 3] {
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
			.map(|x| TriangleVertex { position: [x.sin(), x.cos()] })
	}

	pub fn update(&self, time: f32) {
		self.0.write().unwrap().copy_from_slice(TriangleModel::state(time).as_slice());
	}
}

impl Deref for TriangleModel {
	type Target = Subbuffer<[TriangleVertex]>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
