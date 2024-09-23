use rangemap::RangeSet;
use std::ops::{Deref, DerefMut, Range};
use vulkano_bindless_shaders::descriptor::DescriptorIndex;

pub fn range_to_descriptor_index(range: Range<DescriptorIndex>) -> impl Iterator<Item = DescriptorIndex> {
	(range.start.to_u32()..range.end.to_u32()).map(|i| unsafe { DescriptorIndex::new(i).unwrap() })
}

pub struct DescriptorIndexRangeSet(pub RangeSet<DescriptorIndex>);

impl DescriptorIndexRangeSet {
	pub fn new() -> Self {
		Self(RangeSet::new())
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
