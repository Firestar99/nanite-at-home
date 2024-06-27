use core::mem::size_of;
use glam::{Vec2, Vec3, Vec3A};
use spirv_std::image::Image2d;
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::BufferContent;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::Buffer;

#[repr(C)]
#[derive(Copy, Clone, BufferContent)]
pub struct OpaqueModel {
	pub vertex_buffer: StrongDesc<Buffer<[OpaqueVertex]>>,
	pub index_buffer: StrongDesc<Buffer<[u32]>>,
	pub triangle_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, BufferContent)]
pub struct OpaqueVertex {
	pub position: Vec3A,
	pub tex_coord: Vec2,
	pub tex_id: StrongDesc<Image2d>,
}
const_assert_eq!(size_of::<OpaqueVertex>(), 32);

impl OpaqueVertex {
	#[inline]
	pub const fn new(position: Vec3, tex_coord: Vec2, tex_id: StrongDesc<Image2d>) -> Self {
		Self {
			position: Vec3A::new(position.x, position.y, position.z),
			tex_coord,
			tex_id,
		}
	}
}
