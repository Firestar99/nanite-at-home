use glam::{IVec2, UVec2, Vec2};

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Rect2 {
	pub origin: Vec2,
	pub extent: Vec2,
}

#[cfg_attr(not(target_arch = "spirv"), derive(Hash))]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct IRect2 {
	pub origin: IVec2,
	pub extent: UVec2,
}
