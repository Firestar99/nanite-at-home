use std::cmp::Ordering;
use std::num::Wrapping;
use std::ops::Deref;

use static_assertions::const_assert_eq;

/// Timestamp is represented by [`Wrapping`]`<u32>`.
///
/// # Ordering
/// Timestamp are only partially ordered using [`Timestamp::compare_wrapping`] due to being able to wrap around. If two timestamps are within `0x3FFFFFFFu32`
/// (or one quarter of [`u32::MAX`]) of each other, they can be compared. Otherwise, the timestamps are seen as too far apart, and that timestamp wrapping could
/// cause an issue. This breaks transitivity (e.g. a < b and b < c then a < c) which is required by [`PartialOrd`], and thus cannot be implemented for this type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Timestamp(pub Wrapping<u32>);

impl Timestamp {
	#[inline]
	pub fn new(value: u32) -> Self {
		Self(Wrapping(value))
	}

	pub fn get(&self) -> Wrapping<u32> {
		self.0
	}
}

impl Deref for Timestamp {
	type Target = Wrapping<u32>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<Timestamp> for Wrapping<u32> {
	fn from(value: Timestamp) -> Self {
		value.0
	}
}

impl From<Timestamp> for u32 {
	fn from(value: Timestamp) -> Self {
		value.0 .0
	}
}

impl Timestamp {
	/// see [`Timestamp`] #Ordering
	pub fn compare_wrapping(&self, other: &Self) -> Option<Ordering> {
		// assert same valid value range
		const_assert_eq!(0xFFFFFFFFu32 - 0xC0000000u32, 0x3FFFFFFFu32);
		// assert invalid range -1 == both valid ranges
		const_assert_eq!(0xBFFFFFFFu32 - 0x40000000u32 - 1, 2 * 0x3FFFFFFFu32);

		// these need to be constants unfortunately
		match (**other - **self).0 {
			0 => Some(Ordering::Equal),
			1..=0x3FFFFFFF => Some(Ordering::Less),
			0x40000000..=0xBFFFFFFF => None,
			0xC0000000..=0xFFFFFFFF => Some(Ordering::Greater),
		}
	}
}
