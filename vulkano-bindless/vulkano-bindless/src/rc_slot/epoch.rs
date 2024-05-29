use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::num::Wrapping;
use std::ops::Deref;

use static_assertions::const_assert_eq;

/// Epoch is represented by [`Wrapping`]`<u32>`.
///
/// # Ordering
/// Epochs are technically only partially ordered using [`Epoch::compare_wrapping`] due to being able to wrap
/// around. If two timestamps are within `0x3FFFFFFFu32` (or one quarter of [`u32::MAX`]) of each other, they can be
/// compared. Otherwise, the timestamps are considered too far apart for safe comparison and will return an error.
///
/// This erroring behaviour does break transitivity (e.g. a < b and b < c then a < c), which is required by
/// [`PartialOrd`], but we implement both [`PartialOrd`] and [`Ord`] anyway to be able to use a [`rangemap::RangeMap`]
/// with Timestamps. In case of a comparison error, [`PartialOrd`] returns None and [`Ord`] panics.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Epoch(pub Wrapping<u32>);

impl Epoch {
	#[inline]
	pub fn new(value: u32) -> Self {
		Self(Wrapping(value))
	}

	pub fn get(&self) -> Wrapping<u32> {
		self.0
	}
}

impl Deref for Epoch {
	type Target = Wrapping<u32>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<Epoch> for Wrapping<u32> {
	fn from(value: Epoch) -> Self {
		value.0
	}
}

impl From<Epoch> for u32 {
	fn from(value: Epoch) -> Self {
		value.0 .0
	}
}

impl Display for Epoch {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.0, f)
	}
}

impl Epoch {
	/// see [`Epoch`] #Ordering
	#[inline]
	pub fn compare_wrapping(&self, other: &Self) -> Result<Ordering, EpochCompareError> {
		// assert same valid value range
		const_assert_eq!(0xFFFFFFFFu32 - 0xC0000000u32, 0x3FFFFFFFu32);
		// assert invalid range -1 == both valid ranges
		const_assert_eq!(0xBFFFFFFFu32 - 0x40000000u32 - 1, 2 * 0x3FFFFFFFu32);

		// these need to be constants unfortunately
		let diff = (other.0 - self.0).0;
		match diff {
			0 => Ok(Ordering::Equal),
			1..=0x3FFFFFFF => Ok(Ordering::Less),
			0x40000000..=0xBFFFFFFF => Err(EpochCompareError::WrappingOverflow(*self, *other, diff)),
			0xC0000000..=0xFFFFFFFF => Ok(Ordering::Greater),
		}
	}
}

pub enum EpochCompareError {
	WrappingOverflow(Epoch, Epoch, u32),
}

impl Debug for EpochCompareError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			EpochCompareError::WrappingOverflow(first, other, diff) => {
				f.write_fmt(format_args!("{} and {} have a difference of {}, which is considered too far apart to reasonable differentiate order", first, other, diff))
			}
		}
	}
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd<Self> for Epoch {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.compare_wrapping(other).ok()
	}
}

impl Ord for Epoch {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		self.compare_wrapping(other).unwrap()
	}
}
