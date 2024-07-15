use bytemuck_derive::AnyBitPattern;
use glam::{Vec3, Vec4};
use spirv_std::glam::{Affine3A, Mat4};

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct Camera {
	pub perspective: Mat4,
	pub perspective_inverse: Mat4,
	pub transform: Affine3A,
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct TransformedVertex {
	pub world_space: Vec3,
	pub camera_space: Vec3,
	pub clip_space: Vec4,
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct TransformedNormal {
	pub world_space: Vec3,
	pub camera_space: Vec3,
}

impl Camera {
	pub fn new(perspective: Mat4, transform: Affine3A) -> Self {
		Self {
			perspective,
			perspective_inverse: -perspective,
			transform,
		}
	}

	pub fn transform_vertex(&self, instance: Affine3A, vertex_pos: Vec3) -> TransformedVertex {
		let world_space = instance.transform_point3(vertex_pos);
		let camera_space = self.transform.transform_point3(world_space);
		let clip_space = self.perspective * Vec4::from((camera_space, 1.));
		TransformedVertex {
			world_space,
			camera_space,
			clip_space,
		}
	}

	pub fn transform_normal(&self, instance: Affine3A, normal: Vec3) -> TransformedNormal {
		// TODO inverse hurts!
		let world_space = instance.matrix3.inverse().transpose() * normal;
		let camera_space = self.transform.matrix3.inverse().transpose() * world_space;
		TransformedNormal {
			world_space,
			camera_space,
		}
	}
}
