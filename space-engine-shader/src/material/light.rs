use crate::material::radiance::Radiance;
use glam::Vec3;
use vulkano_bindless_macros::BufferContent;

#[derive(Copy, Clone, BufferContent)]
pub struct DirectionalLight {
	pub direction: Vec3,
	pub color: Radiance,
}

#[derive(Copy, Clone, BufferContent)]
pub struct PointLight {
	pub position: Vec3,
	pub color: Radiance,
}
