use core::fmt::{Debug, Formatter};
use glam::Vec4;
use num_enum::{FromPrimitive, IntoPrimitive};

#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Hash, FromPrimitive, IntoPrimitive)]
pub enum ColorEnum {
	Red,
	Cyan,
	Yellow,
	Black,
	#[default]
	Unknown,
}

impl Debug for ColorEnum {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		match self {
			ColorEnum::Red => f.write_str("R"),
			ColorEnum::Cyan => f.write_str("C"),
			ColorEnum::Yellow => f.write_str("Y"),
			ColorEnum::Black => f.write_str("B"),
			ColorEnum::Unknown => f.write_str("U"),
		}
	}
}

impl ColorEnum {
	const COLORS: &'static [Vec4] = &[
		Vec4::new(1., 0., 0., 1.),
		Vec4::new(0., 1., 1., 1.),
		Vec4::new(1., 1., 0., 1.),
		Vec4::new(0., 0., 0., 0.),
		Vec4::new(1., 1., 1., 1.),
	];

	pub fn parse(color: Vec4) -> Self {
		for (i, value) in Self::COLORS.iter().enumerate() {
			if (color - value).length() <= 0.01 {
				return Self::from_primitive(i as u32);
			}
		}
		Self::Unknown
	}

	pub fn color(&self) -> Vec4 {
		Self::COLORS[u32::from(*self) as usize]
	}
}
