use core::ops::Range;
use vulkano_bindless_macros::BufferContentPlain;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContentPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct RangeU32 {
	pub start: u32,
	pub end: u32,
}

impl RangeU32 {
	pub fn new(start: u32, end: u32) -> Self {
		Self { start, end }
	}
}

impl From<Range<u32>> for RangeU32 {
	fn from(value: Range<u32>) -> Self {
		Self {
			start: value.start,
			end: value.end,
		}
	}
}

impl From<RangeU32> for Range<u32> {
	fn from(value: RangeU32) -> Self {
		value.start..value.end
	}
}
