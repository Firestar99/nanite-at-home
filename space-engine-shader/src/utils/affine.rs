use glam::Affine3A;

pub trait AffineTranspose {
	fn transpose(&self) -> Self;
}

impl AffineTranspose for Affine3A {
	/// Return the transpose of this transform.
	///
	/// Note that if the transform is not invertible the result will be invalid.
	#[inline]
	#[must_use]
	fn transpose(&self) -> Self {
		let matrix3 = self.matrix3.transpose();
		// transform negative translation by the matrix inverse:
		let translation = -(matrix3 * self.translation);

		Self { matrix3, translation }
	}
}
