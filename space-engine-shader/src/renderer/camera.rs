use crate::utils::affine::AffineTranspose;
use bytemuck_derive::AnyBitPattern;
use glam::{Mat4, UVec2, Vec2, Vec3, Vec4, Vec4Swizzles, vec4};
use rust_gpu_bindless_macros::BufferStruct;
use space_asset_shader::affine_transform::AffineTransform;

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct Camera {
	pub clip_from_view: Mat4,
	pub view_from_clip: Mat4,
	pub view_from_world: AffineTransform,
	pub viewport_size: UVec2,
	pub fov_y: f32,
	pub z_near: f32,
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct TransformedPosition {
	pub world_space: Vec3,
	pub view_space: Vec3,
	pub clip_space: Vec4,
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct TransformedNormal {
	pub world_space: Vec3,
	pub view_space: Vec3,
}

impl Camera {
	pub fn new(perspective: Mat4, viewport_size: UVec2, fov_y: f32, z_near: f32, transform: AffineTransform) -> Self {
		Self {
			clip_from_view: perspective,
			view_from_clip: perspective.inverse(),
			view_from_world: transform,
			viewport_size,
			fov_y,
			z_near,
		}
	}

	pub fn new_perspective_rh_y_flip(
		viewport_size: UVec2,
		fov_y: f32,
		z_near: f32,
		z_far: f32,
		transform: AffineTransform,
	) -> Self {
		let projection = Mat4::perspective_rh(fov_y, viewport_size.x as f32 / viewport_size.y as f32, z_near, z_far)
			* Mat4::from_cols(
				vec4(1., 0., 0., 0.),
				vec4(0., -1., 0., 0.),
				vec4(0., 0., 1., 0.),
				vec4(0., 0., 0., 1.),
			);
		Self::new(projection, viewport_size, fov_y, z_near, transform)
	}

	pub fn transform_vertex(&self, world_from_local: AffineTransform, vertex_pos: Vec3) -> TransformedPosition {
		let world_space = world_from_local.affine.transform_point3(vertex_pos);
		let view_space = self.view_from_world.affine.transform_point3_transposed(world_space);
		let clip_space = self.clip_from_view * Vec4::from((view_space, 1.));
		TransformedPosition {
			world_space,
			view_space,
			clip_space,
		}
	}

	pub fn transform_normal(&self, world_from_local: AffineTransform, normal: Vec3) -> TransformedNormal {
		let world_space = world_from_local.normal * normal;
		let view_space = self.view_from_world.normal.transpose() * world_space;
		TransformedNormal {
			world_space,
			view_space,
		}
	}

	/// Reconstruct positions from fragment position [0, 1] and depth value
	pub fn reconstruct_from_depth(&self, fragment_pos: Vec2, depth: f32) -> TransformedPosition {
		let clip_space = Vec4::from((fragment_pos * 2. - 1., depth, 1.));
		let camera_space = self.view_from_clip * clip_space;
		let view_space = camera_space.xyz() / camera_space.w;
		let world_space = self.view_from_world.affine.transform_point3(view_space);
		TransformedPosition {
			world_space,
			view_space,
			clip_space,
		}
	}

	pub fn reconstruct_direction(&self, fragment_pos: Vec2) -> TransformedNormal {
		let clip_pos = fragment_pos * 2. - 1.;
		let clip_space = Vec4::from((clip_pos, (1. - clip_pos.length()).max(0.), 1.));
		let camera_space = (self.view_from_clip * clip_space).xyz().normalize();
		let world_space = self.view_from_world.normal * camera_space;
		TransformedNormal {
			world_space,
			view_space: camera_space,
		}
	}
}
