use glam::{Affine3A, Mat3A, Vec3};
use vulkano_bindless_macros::{assert_transfer_size, BufferContent};

/// Affine transformation like [`Affine3A`] but also stores a matrix to transform normals.
#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContent)]
pub struct AffineTransform {
	pub affine: Affine3A,
	pub normals: Mat3A,
}
assert_transfer_size!(AffineTransform, 24 * 4);

impl AffineTransform {
	pub fn new(transform: Affine3A) -> Self {
		Self {
			affine: transform,
			normals: transform.matrix3.inverse().transpose(),
		}
	}

	pub fn translation(&self) -> Vec3 {
		Vec3::from(self.affine.translation)
	}
}
