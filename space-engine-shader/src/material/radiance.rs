use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use glam::Vec3;
use rust_gpu_bindless_macros::BufferStruct;

#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct Radiance(pub Vec3);

impl Radiance {
	pub fn tone_map_reinhard(&self) -> Vec3 {
		self.0 / (self.0 + Vec3::splat(1.0))
	}
}

impl From<Vec3> for Radiance {
	fn from(value: Vec3) -> Self {
		Radiance(value)
	}
}

impl Add<Radiance> for Radiance {
	type Output = Radiance;

	fn add(self, rhs: Radiance) -> Self::Output {
		Radiance(self.0 + rhs.0)
	}
}

impl AddAssign<Radiance> for Radiance {
	fn add_assign(&mut self, rhs: Radiance) {
		*self = *self + rhs
	}
}

impl Sub<Radiance> for Radiance {
	type Output = Radiance;

	fn sub(self, rhs: Radiance) -> Self::Output {
		Radiance(self.0 - rhs.0)
	}
}

impl SubAssign<Radiance> for Radiance {
	fn sub_assign(&mut self, rhs: Radiance) {
		*self = *self - rhs
	}
}

impl Mul<f32> for Radiance {
	type Output = Radiance;

	fn mul(self, rhs: f32) -> Self::Output {
		Radiance(self.0 * rhs)
	}
}

impl MulAssign<f32> for Radiance {
	fn mul_assign(&mut self, rhs: f32) {
		*self = *self * rhs
	}
}

impl Div<f32> for Radiance {
	type Output = Radiance;

	fn div(self, rhs: f32) -> Self::Output {
		Radiance(self.0 / rhs)
	}
}

impl DivAssign<f32> for Radiance {
	fn div_assign(&mut self, rhs: f32) {
		*self = *self / rhs
	}
}
