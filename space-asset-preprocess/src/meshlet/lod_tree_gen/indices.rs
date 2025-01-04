use std::hash::Hash;
use std::ops::Deref;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct IndexPair<T: Copy + Ord>((T, T));

impl<T: Copy + Ord> IndexPair<T> {
	pub fn new(a: T, b: T) -> Self {
		if a < b {
			Self((a, b))
		} else {
			Self((b, a))
		}
	}

	pub fn to_array(&self) -> [T; 2] {
		[self.0 .0, self.0 .1]
	}

	pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
		self.to_array().into_iter()
	}
}

impl<T: Copy + Ord> Deref for IndexPair<T> {
	type Target = (T, T);

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MeshletId(pub u32);

impl Deref for MeshletId {
	type Target = u32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
