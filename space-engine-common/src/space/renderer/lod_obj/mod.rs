use core::mem::size_of;

use bytemuck_derive::AnyBitPattern;
use glam::{Vec3, Vec3A};
use static_assertions::const_assert_eq;

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct VertexInput {
	pub position: Vec3A,
}

const_assert_eq!(size_of::<VertexInput>(), 16);

impl VertexInput {
	pub const fn new(position: Vec3) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
		}
	}
}
