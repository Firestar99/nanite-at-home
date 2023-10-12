use spirv_std::glam::Vec3A;

#[derive(Copy, Clone)]
pub struct VertexInput {
	pub position: Vec3A,
}

impl VertexInput {
	pub const fn new(position: Vec3A) -> Self {
		Self {
			position
		}
	}
}
