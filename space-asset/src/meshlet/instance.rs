use core::mem;
use glam::{Affine3A, Mat3A, Vec3A};
use static_assertions::const_assert_eq;
use vulkano_bindless_macros::BufferContent;

/// same as `Affine3A::from_cols_array(&transform)` but doesn't use slices
pub const fn affine3a_from_cols_array(transform: [f32; 12]) -> Affine3A {
	Affine3A {
		matrix3: Mat3A::from_cols_array(&[
			transform[0],
			transform[1],
			transform[2],
			transform[3],
			transform[4],
			transform[5],
			transform[6],
			transform[7],
			transform[8],
		]),
		translation: Vec3A::from_array([transform[9], transform[10], transform[11]]),
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContent)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MeshletInstance {
	pub transform: [f32; 12],
}
const_assert_eq!(mem::size_of::<MeshletInstance>(), 12 * 4);

impl MeshletInstance {
	pub fn new(transform: Affine3A) -> Self {
		Self {
			transform: transform.to_cols_array(),
		}
	}

	pub fn transform(&self) -> Affine3A {
		affine3a_from_cols_array(self.transform)
	}
}

impl Default for MeshletInstance {
	fn default() -> Self {
		Self::new(Affine3A::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_affine3a_from_cols_array() {
		let affine3a = Affine3A::from_cols_array(&[0., 1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.]);
		assert_eq!(affine3a, affine3a_from_cols_array(affine3a.to_cols_array()));
	}
}
