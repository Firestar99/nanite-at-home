use smallvec::{Array, SmallVec};
use std::fmt::{Debug, Formatter};
use std::slice::Iter;

pub struct SortedSmallVec<A: Array>(SmallVec<A>)
where
	A::Item: Ord;

impl<A: Array> Clone for SortedSmallVec<A>
where
	A::Item: Ord + Clone,
{
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<A: Array> Debug for SortedSmallVec<A>
where
	A::Item: Ord + Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("SortedSmallVec").field(&self.0).finish()
	}
}

impl<A: Array> Default for SortedSmallVec<A>
where
	A::Item: Ord,
{
	fn default() -> Self {
		Self::new()
	}
}

impl<A: Array> SortedSmallVec<A>
where
	A::Item: Ord,
{
	pub fn new() -> Self {
		Self(SmallVec::new())
	}

	pub fn insert(&mut self, element: A::Item) -> bool {
		match self.0.binary_search(&element) {
			Ok(_) => false,
			Err(i) => {
				self.0.insert(i, element);
				true
			}
		}
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn capacity(&self) -> usize {
		self.0.capacity()
	}

	pub fn iter(&self) -> Iter<'_, <A as Array>::Item> {
		self.0.iter()
	}
}
