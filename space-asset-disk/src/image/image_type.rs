#![allow(non_camel_case_types)]

use num_enum::{IntoPrimitive, TryFromPrimitive};
use rkyv::{Archive, Deserialize, Serialize};

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Archive, Serialize, Deserialize)]
pub enum ImageType {
	/// Single channel of values.
	/// Usually represented by R8 or BC4.
	R_VALUE,
	/// Two channels of values which are compressed independently. Optimal for normal maps.
	/// Usually represented by RG8 or BC5.
	RG_VALUE,
	/// Four channels of linear values that are considered to correlate somewhat for compression.
	/// May cause compression artifacts for highly non-correlated values, like normals.
	/// Usually represented by RGBA8 or BC7.
	RGBA_LINEAR,
	/// Four channels of color in sRGB color space.
	/// Usually represented by RGBA8 or BC7 in sRGB color space.
	RGBA_COLOR,
}

impl ImageType {
	pub const IMAGE_TYPE_COUNT: u32 = 4;

	pub const fn try_from_const(value: u32) -> ImageType {
		[
			ImageType::R_VALUE,
			ImageType::RG_VALUE,
			ImageType::RGBA_LINEAR,
			ImageType::RGBA_COLOR,
		][value as usize]
	}

	pub const fn channels(&self) -> u32 {
		match self {
			ImageType::R_VALUE => 1,
			ImageType::RG_VALUE => 2,
			ImageType::RGBA_LINEAR => 4,
			ImageType::RGBA_COLOR => 4,
		}
	}
}
