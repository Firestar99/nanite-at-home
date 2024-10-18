use rangemap::RangeSet;
use rust_gpu_bindless_shaders::descriptor::DescriptorIndex;
use std::ops::{Deref, DerefMut, Range};

pub fn range_to_descriptor_index(range: Range<DescriptorIndex>) -> impl Iterator<Item = DescriptorIndex> {
	(range.start.to_u32()..range.end.to_u32()).map(|i| unsafe { DescriptorIndex::new(i).unwrap() })
}

#[derive(Clone, Debug)]
pub struct DescriptorIndexRangeSet(pub RangeSet<DescriptorIndex>);

impl Default for DescriptorIndexRangeSet {
	fn default() -> Self {
		Self::new()
	}
}

impl DescriptorIndexRangeSet {
	pub fn new() -> Self {
		Self(RangeSet::new())
	}

	pub fn insert(&mut self, index: DescriptorIndex) {
		let next = unsafe { DescriptorIndex::new(index.to_u32() + 1).unwrap() };
		self.0.insert(index..next);
	}

	pub fn into_inner(self) -> RangeSet<DescriptorIndex> {
		self.0
	}

	pub fn iter_ranges(&self) -> impl Iterator<Item = Range<DescriptorIndex>> + '_ {
		self.0.iter().cloned()
	}

	pub fn iter(&self) -> impl Iterator<Item = DescriptorIndex> + '_ {
		self.iter_ranges().flat_map(range_to_descriptor_index)
	}
}

impl From<RangeSet<DescriptorIndex>> for DescriptorIndexRangeSet {
	fn from(value: RangeSet<DescriptorIndex>) -> Self {
		Self(value)
	}
}

impl Deref for DescriptorIndexRangeSet {
	type Target = RangeSet<DescriptorIndex>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for DescriptorIndexRangeSet {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[cfg(test)]
mod tests {
	use crate::backend::range_set::DescriptorIndexRangeSet;
	use rust_gpu_bindless_shaders::descriptor::DescriptorIndex;

	#[test]
	fn test_range_set() {
		unsafe {
			let indices = [1, 2, 3, 4, 6, 7, 9, 42, 69];
			let mut set = DescriptorIndexRangeSet::new();
			for i in indices {
				set.insert(DescriptorIndex::new(i).unwrap());
			}
			assert_eq!(&indices[..], set.iter().map(|i| i.to_u32()).collect::<Vec<_>>());
		}
	}
}
