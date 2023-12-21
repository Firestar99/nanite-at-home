use core::mem::size_of;

use bytemuck_derive::AnyBitPattern;
use glam::{Vec2, Vec3, Vec3A};
use static_assertions::const_assert_eq;

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct ModelVertex {
	pub position: Vec3A,
	pub tex_coord: Vec2,
}

const_assert_eq!(size_of::<ModelVertex>(), 32);

impl ModelVertex {
	pub const fn new(position: Vec3, tex_coord: Vec2) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
			tex_coord,
		}
	}
}
