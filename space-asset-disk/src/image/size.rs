use glam::UVec2;
use rkyv::{Archive, Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
pub struct Size {
	pub width: u32,
	pub height: u32,
}

impl Size {
	pub const fn new(width: u32, height: u32) -> Self {
		Self { width, height }
	}
}

impl From<UVec2> for Size {
	fn from(value: UVec2) -> Self {
		Self::new(value.x, value.y)
	}
}

impl From<Size> for UVec2 {
	fn from(value: Size) -> Self {
		Self::new(value.width, value.height)
	}
}
