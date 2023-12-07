use core::mem::size_of;

use bytemuck_derive::AnyBitPattern;
use glam::{Vec3, Vec3A};
use static_assertions::const_assert_eq;

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct ModelVertex {
	pub position: Vec3A,
}

const_assert_eq!(size_of::<ModelVertex>(), 16);

impl ModelVertex {
	pub const fn new(position: Vec3) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
		}
	}
}
