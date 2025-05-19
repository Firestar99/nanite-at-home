use glam::{Affine3A, Vec3, Vec3A};

pub trait AffineTranspose {
	#[must_use]
	fn transpose(&self) -> Self;
	fn transform_point3_transposed(&self, rhs: Vec3) -> Vec3;
	fn transform_vector3_transposed(&self, rhs: Vec3) -> Vec3;
	fn transform_point3a_transposed(&self, rhs: Vec3A) -> Vec3A;
	fn transform_vector3a_transposed(&self, rhs: Vec3A) -> Vec3A;
}

impl AffineTranspose for Affine3A {
	/// Return the transpose of this transform.
	///
	/// Note that if the transform is not invertible the result will be invalid.
	#[inline]
	fn transpose(&self) -> Self {
		let matrix3 = self.matrix3.transpose();
		// transform negative translation by the matrix inverse:
		let translation = -(matrix3 * self.translation);

		Self { matrix3, translation }
	}

	fn transform_point3_transposed(&self, rhs: Vec3) -> Vec3 {
		let rhs = Vec3A::from(rhs) - self.translation;
		let mat = self.matrix3.transpose();
		((mat.x_axis * rhs.x) + (mat.y_axis * rhs.y) + (mat.z_axis * rhs.z)).into()
	}

	fn transform_vector3_transposed(&self, rhs: Vec3) -> Vec3 {
		let mat = self.matrix3.transpose();
		((mat.x_axis * rhs.x) + (mat.y_axis * rhs.y) + (mat.z_axis * rhs.z)).into()
	}

	fn transform_point3a_transposed(&self, rhs: Vec3A) -> Vec3A {
		self.matrix3.transpose() * (rhs - self.translation)
	}

	fn transform_vector3a_transposed(&self, rhs: Vec3A) -> Vec3A {
		self.matrix3.transpose() * rhs
	}
}

#[cfg(test)]
mod tests {
	use crate::utils::affine::AffineTranspose;
	use glam::{Affine3A, Quat, Vec3, Vec3A};
	use std::f32::consts::PI;

	#[test]
	fn test_transpose() {
		let affine = Affine3A::from_rotation_translation(
			Quat::from_axis_angle(Vec3::new(0., 1., 0.), PI / 2.),
			Vec3::new(42., 69., -1234.),
		);

		assert!(
			affine.abs_diff_eq(affine.transpose().transpose(), 0.01),
			"double transpose"
		);
		assert!(
			affine.inverse().abs_diff_eq(affine.transpose(), 0.01),
			"transpose eq inverse"
		);
	}

	#[test]
	fn test_transform_transposed() {
		let affine = Affine3A::from_rotation_translation(
			Quat::from_axis_angle(Vec3::new(0., 1., 0.), PI / 2.),
			Vec3::new(67., 69., -1234.),
		);
		let base = Vec3::new(567., -87., -43.);
		let point = affine.inverse().transform_point3(base);
		let vector = affine.inverse().transform_vector3(base);

		assert!(point.abs_diff_eq(affine.transform_point3_transposed(base), 0.01));
		assert!(vector.abs_diff_eq(affine.transform_vector3_transposed(base), 0.01));
		assert!(Vec3A::from(point).abs_diff_eq(affine.transform_point3a_transposed(base.into()), 0.01));
		assert!(Vec3A::from(vector).abs_diff_eq(affine.transform_vector3a_transposed(base.into()), 0.01));
	}
}
