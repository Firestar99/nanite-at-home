use core::mem::size_of;
use glam::{Vec2, Vec3, Vec3A};
use spirv_std::image::Image2d;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;

#[repr(C)]
#[derive(Copy, Clone, DescStruct)]
pub struct ModelVertex<'a> {
	pub position: Vec3A,
	pub tex_coord: Vec2,
	pub tex_id: StrongDesc<'a, Image2d>,
}

const_assert_eq!(size_of::<ModelVertex>(), 32);

impl<'a> ModelVertex<'a> {
	#[inline]
	pub const fn new(position: Vec3, tex_coord: Vec2, tex_id: StrongDesc<'a, Image2d>) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
			tex_coord,
			tex_id,
		}
	}
}
