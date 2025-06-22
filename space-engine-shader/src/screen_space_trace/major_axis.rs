use core::ops::Deref;
use glam::{IVec2, UVec2, Vec2};
use num_enum::{IntoPrimitive, UnsafeFromPrimitive};
use rust_gpu_bindless_macros::BufferStruct;

/// The major axis is the larger axis of a 2D vector (X, Y) with a directional component (N for negative)
#[repr(u32)]
#[derive(Copy, Clone, Debug, UnsafeFromPrimitive, IntoPrimitive)]
pub enum MajorAxis {
	X,
	Y,
	NX,
	NY,
}

impl MajorAxis {
	pub fn new(vec: Vec2) -> Self {
		let abs_x = vec.x.abs();
		let abs_y = vec.y.abs();
		if abs_x > abs_y {
			if vec.x > 0. {
				MajorAxis::X
			} else {
				MajorAxis::NX
			}
		} else {
			if vec.y > 0. {
				MajorAxis::Y
			} else {
				MajorAxis::NY
			}
		}
	}

	pub fn get_minor(&self, dir: Vec2) -> f32 {
		match self {
			MajorAxis::X | MajorAxis::NX => dir.y / dir.x,
			MajorAxis::Y | MajorAxis::NY => dir.x / dir.y,
		}
	}

	pub fn major_dir(&self) -> IVec2 {
		match self {
			MajorAxis::X => IVec2::new(1, 0),
			MajorAxis::Y => IVec2::new(0, 1),
			MajorAxis::NX => IVec2::new(-1, 0),
			MajorAxis::NY => IVec2::new(0, -1),
		}
	}

	pub fn major_dir_abs(&self) -> UVec2 {
		self.major_dir().abs().as_uvec2()
	}

	pub fn minor_dir(&self) -> IVec2 {
		match self {
			MajorAxis::X => IVec2::new(0, 1),
			MajorAxis::Y => IVec2::new(1, 0),
			MajorAxis::NX => IVec2::new(0, -1),
			MajorAxis::NY => IVec2::new(-1, 0),
		}
	}

	pub fn minor_dir_abs(&self) -> UVec2 {
		self.minor_dir().abs().as_uvec2()
	}
}

mod major_axis_transfer {
	use super::*;
	use bytemuck_derive::AnyBitPattern;
	use rust_gpu_bindless_shaders::buffer_content::BufferStructPlain;

	#[repr(C)]
	#[derive(Copy, Clone, AnyBitPattern)]
	pub struct MajorAxisTransfer(u32);

	unsafe impl BufferStructPlain for MajorAxis {
		type Transfer = MajorAxisTransfer;

		unsafe fn write(self) -> Self::Transfer {
			MajorAxisTransfer(self.into())
		}

		unsafe fn read(from: Self::Transfer) -> Self {
			unsafe { MajorAxis::unchecked_transmute_from(from.0) }
		}
	}
}

/// TraceDirection represents a normalised [`Vec2`] direction as a [`MajorAxis`] with a minor factor
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct TraceDirection {
	/// The major axis is the larger axis of a 2D vector
	pub major: MajorAxis,
	/// How much the minor axis progresses when the major axis is incremented by 1. Must be `-1.0 <= minor <= 1.0`.
	pub minor: f32,
}

impl TraceDirection {
	pub fn new(dir: Vec2) -> Self {
		let major = MajorAxis::new(dir);
		let minor = major.get_minor(dir);
		Self { major, minor }
	}

	pub fn minor_factor(&self) -> f32 {
		self.minor
	}

	pub fn minor_vec(&self) -> Vec2 {
		self.minor_dir().as_vec2() * self.minor_factor()
	}

	pub fn to_vec2(&self) -> Vec2 {
		self.major_dir().as_vec2() + self.minor_vec()
	}
}

impl Deref for TraceDirection {
	type Target = MajorAxis;

	fn deref(&self) -> &Self::Target {
		&self.major
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use approx::assert_relative_eq;
	use glam::Mat2;
	use std::f32::consts::PI;

	#[test]
	pub fn rotating_positive() {
		rotating(Vec2::new(1., 0.5));
	}

	#[test]
	pub fn rotating_negative() {
		rotating(Vec2::new(1., -0.3));
	}

	pub fn rotating(base_vec: Vec2) {
		for quarter_rot in 0..4 {
			println!("quarter_rot: {}", quarter_rot);
			let rot = Mat2::from_angle(quarter_rot as f32 * PI / 2.);
			let vec = rot * base_vec;
			let trace = TraceDirection::new(vec);
			println!("trace: {:?}", trace);
			assert_relative_eq!(trace.to_vec2(), vec, epsilon = 0.01);
		}
	}
}
