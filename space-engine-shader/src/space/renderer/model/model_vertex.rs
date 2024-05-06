use bytemuck_derive::AnyBitPattern;
use core::mem::size_of;
use glam::{Vec2, Vec3, Vec3A};
use static_assertions::const_assert_eq;
use vulkano_bindless_shaders::descriptor::{SampledImage2D, WeakDesc};

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct ModelVertex {
	pub position: Vec3A,
	pub tex_coord: Vec2,
	pub tex_id: WeakDesc<SampledImage2D>,
}

const_assert_eq!(size_of::<ModelVertex>(), 32);

impl ModelVertex {
	#[inline]
	pub const fn new(position: Vec3, tex_coord: Vec2, tex_id: WeakDesc<SampledImage2D>) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
			tex_coord,
			tex_id,
		}
	}
}
