use spirv_std::glam::Vec2;

#[derive(Copy, Clone)]
pub struct VertexInput {
	pub position: Vec2,
}

impl VertexInput {
	pub const fn new(position: Vec2) -> Self {
		Self {
			position
		}
	}
}
