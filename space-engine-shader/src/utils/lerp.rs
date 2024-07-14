pub trait Lerp {
	type S;
	/// Performs a linear interpolation between `self` and `rhs` based on the value `s`.
	///
	/// When `s` is `0.0`, the result will be equal to `self`.  When `s` is `1.0`, the result
	/// will be equal to `rhs`. When `s` is outside of range `[0, 1]`, the result is linearly
	/// extrapolated.
	#[doc(alias = "mix")]
	fn lerp(self, rhs: Self, s: Self::S) -> Self;
}

impl Lerp for f32 {
	type S = f32;

	#[inline]
	fn lerp(self, rhs: Self, s: Self::S) -> Self {
		self + ((rhs - self) * s)
	}
}

impl Lerp for f64 {
	type S = f64;

	#[inline]
	fn lerp(self, rhs: Self, s: Self::S) -> Self {
		self + ((rhs - self) * s)
	}
}
