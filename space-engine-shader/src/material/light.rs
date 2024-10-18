use crate::material::radiance::Radiance;
use glam::Vec3;
use rust_gpu_bindless_macros::{assert_transfer_size, BufferContent};

#[derive(Copy, Clone, Debug, BufferContent)]
pub struct DirectionalLight {
	pub direction: Vec3,
	pub color: Radiance,
}
assert_transfer_size!(DirectionalLight, 6 * 4);

#[derive(Copy, Clone, BufferContent)]
pub struct PointLight {
	pub position: Vec3,
	pub color: Radiance,
}
assert_transfer_size!(PointLight, 6 * 4);
