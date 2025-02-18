use glam::{IVec2, UVec2, Vec2};

#[derive(Copy, Clone, Default, Debug)]
pub struct Rect2 {
	pub origin: Vec2,
	pub extent: Vec2,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct IRect2 {
	pub origin: IVec2,
	pub extent: UVec2,
}
