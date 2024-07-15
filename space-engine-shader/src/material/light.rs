use bytemuck_derive::AnyBitPattern;
use glam::Vec3;

#[derive(Copy, Clone, AnyBitPattern)]
pub struct DirectionalLight {
	pub direction: Vec3,
	pub color: Vec3,
}

#[derive(Copy, Clone, AnyBitPattern)]
pub struct PointLight {
	pub position: Vec3,
	pub color: Vec3,
}
