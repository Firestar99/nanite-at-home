use crate::utils::affine::AffineTranspose;
use bytemuck_derive::AnyBitPattern;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use rust_gpu_bindless_macros::BufferStruct;
use space_asset_shader::affine_transform::AffineTransform;

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct Camera {
	pub perspective: Mat4,
	pub perspective_inverse: Mat4,
	pub transform: AffineTransform,
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct TransformedPosition {
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
	pub fn new(perspective: Mat4, transform: AffineTransform) -> Self {
		Self {
			perspective,
			perspective_inverse: perspective.inverse(),
			transform,
		}
	}

	pub fn transform_vertex(&self, instance: AffineTransform, vertex_pos: Vec3) -> TransformedPosition {
		let world_space = instance.affine.transform_point3(vertex_pos);
		let camera_space = self.transform.affine.transform_point3_transposed(world_space);
		let clip_space = self.perspective * Vec4::from((camera_space, 1.));
		TransformedPosition {
			world_space,
			camera_space,
			clip_space,
		}
	}

	pub fn transform_normal(&self, instance: AffineTransform, normal: Vec3) -> TransformedNormal {
		let world_space = instance.normals * normal;
		let camera_space = self.transform.normals.transpose() * world_space;
		TransformedNormal {
			world_space,
			camera_space,
		}
	}

	/// Reconstruct positions from fragment position [0, 1] and depth value
	pub fn reconstruct_from_depth(&self, fragment_pos: Vec2, depth: f32) -> TransformedPosition {
		let clip_space = Vec4::from((fragment_pos * 2. - 1., depth, 1.));
		let camera_space = self.perspective_inverse * clip_space;
		let camera_space = camera_space.xyz() / camera_space.w;
		let world_space = self.transform.affine.transform_point3(camera_space);
		TransformedPosition {
			world_space,
			camera_space,
			clip_space,
		}
	}

	pub fn reconstruct_direction(&self, fragment_pos: Vec2) -> TransformedNormal {
		let clip_pos = fragment_pos * 2. - 1.;
		let clip_space = Vec4::from((clip_pos, (1. - clip_pos.length()).max(0.), 1.));
		let camera_space = (self.perspective_inverse * clip_space).xyz().normalize();
		let world_space = self.transform.normals * camera_space;
		TransformedNormal {
			world_space,
			camera_space,
		}
	}
}
