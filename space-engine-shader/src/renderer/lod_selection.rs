use core::fmt::{Debug, Formatter};
use rust_gpu_bindless_macros::BufferStructPlain;

pub const LOD_SELECTION_NANITE: i32 = -1;

/// Nanite is -1
/// Static is >0
#[derive(Copy, Clone, BufferStructPlain)]
pub struct LodSelection(i32);

impl LodSelection {
	#[inline]
	pub fn new_static(level: u32) -> LodSelection {
		Self(level as i32)
	}

	#[inline]
	pub fn new_nanite() -> LodSelection {
		Self(LOD_SELECTION_NANITE)
	}

	#[inline]
	pub fn from(value: i32) -> LodSelection {
		if value < 0 {
			Self(LOD_SELECTION_NANITE)
		} else {
			Self(value)
		}
	}

	#[inline]
	pub fn to_i32(&self) -> i32 {
		self.0
	}

	#[inline]
	pub fn lod_type(&self) -> LodType {
		match self.0 {
			LOD_SELECTION_NANITE => LodType::Nanite,
			_ => LodType::Static,
		}
	}

	#[inline]
	pub fn lod_level_static(&self) -> u32 {
		self.0 as u32
	}
}

impl Debug for LodSelection {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		match self.lod_type() {
			LodType::Nanite => write!(f, "Nanite"),
			LodType::Static => write!(f, "Static({})", self.lod_level_static()),
		}
	}
}

pub enum LodType {
	Nanite,
	Static,
}
