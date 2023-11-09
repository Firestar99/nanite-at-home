use bytemuck_derive::AnyBitPattern;
use glam::Vec3;

#[derive(Copy, Clone, AnyBitPattern)]
pub struct VertexInput {
    pub position: Vec3,
}

impl VertexInput {
    pub const fn new(position: Vec3) -> Self {
        Self {
            position
        }
    }
}
