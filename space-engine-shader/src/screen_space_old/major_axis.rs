use core::ops::Mul;
use glam::{IVec2, Vec2};
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

// TODO do we need neg variants?
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum MajorAxis {
	X,
	XNeg,
	Y,
	YNeg,
}

impl MajorAxis {
	pub fn new(direction: Vec2) -> Self {
		if direction.x.abs() > direction.y.abs() {
			if direction.x > 0. {
				MajorAxis::X
			} else {
				MajorAxis::XNeg
			}
		} else {
			if direction.y > 0. {
				MajorAxis::Y
			} else {
				MajorAxis::YNeg
			}
		}
	}

	pub fn minor_factor(&self, direction: Vec2) -> f32 {
		match self {
			MajorAxis::X => direction.y / direction.x,
			MajorAxis::Y => direction.x / direction.y,
			// FIXME negate here or not?
			MajorAxis::XNeg => -direction.y / direction.x,
			MajorAxis::YNeg => -direction.x / direction.y,
		}
	}
}

impl Mul<IVec2> for MajorAxis {
	type Output = IVec2;

	#[inline]
	fn mul(self, rhs: IVec2) -> Self::Output {
		IVec2::from(match self {
			MajorAxis::X => (rhs.x, rhs.y),
			MajorAxis::Y => (rhs.y, rhs.x),
			MajorAxis::XNeg => (-rhs.x, -rhs.y),
			MajorAxis::YNeg => (-rhs.y, -rhs.x),
		})
	}
}

impl Mul<Vec2> for MajorAxis {
	type Output = Vec2;

	#[inline]
	fn mul(self, rhs: Vec2) -> Self::Output {
		Vec2::from(match self {
			MajorAxis::X => (rhs.x, rhs.y),
			MajorAxis::Y => (rhs.y, rhs.x),
			MajorAxis::XNeg => (-rhs.x, -rhs.y),
			MajorAxis::YNeg => (-rhs.y, -rhs.x),
		})
	}
}
