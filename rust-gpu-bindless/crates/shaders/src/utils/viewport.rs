use glam::UVec2;

/// Viewport defined just like ash's Viewport
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Viewport {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
	pub min_depth: f32,
	pub max_depth: f32,
}

impl Viewport {
	pub fn from_extent(extent: UVec2) -> Self {
		Self {
			x: 0.,
			y: 0.,
			width: extent.x as f32,
			height: extent.y as f32,
			min_depth: 0.,
			max_depth: 1.,
		}
	}
}
