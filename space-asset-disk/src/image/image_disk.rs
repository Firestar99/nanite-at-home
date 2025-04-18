use crate::image::DynImage;
use rkyv::{Archive, Deserialize, Serialize};

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Archive, Serialize, Deserialize)]
pub enum ImageType {
	/// Single channel of values.
	/// Usually represented by R8 or BC4.
	RValue,
	/// Two channels of values which are compressed independently. Optimal for normal maps.
	/// Usually represented by RG8 or BC5.
	RgValue,
	/// Four channels of linear values that are considered to correlate somewhat for compression.
	/// May cause compression artifacts for highly non-correlated values, like normals.
	/// Usually represented by RGBA8 or BC7.
	RgbaLinear,
	/// Four channels of color in sRGB color space.
	/// Usually represented by RGBA8 or BC7 in sRGB color space.
	RgbaColor,
}

impl ImageType {
	pub const IMAGE_TYPE_COUNT: u32 = 4;

	pub const fn from_u32(value: u32) -> ImageType {
		[
			ImageType::RValue,
			ImageType::RgValue,
			ImageType::RgbaLinear,
			ImageType::RgbaColor,
		][value as usize]
	}

	pub const fn to_u32(&self) -> u32 {
		*self as u32
	}

	pub const fn channels(&self) -> u32 {
		match self {
			ImageType::RValue => 1,
			ImageType::RgValue => 2,
			ImageType::RgbaLinear => 4,
			ImageType::RgbaColor => 4,
		}
	}
}

/// An Image that can be stored on disk
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct ImageDisk<const IMAGE_TYPE: u32> {
	pub id: usize,
}

pub type ImageDiskRLinear = ImageDisk<{ ImageType::RValue as u32 }>;
pub type ImageDiskRgLinear = ImageDisk<{ ImageType::RgValue as u32 }>;
pub type ImageDiskRgbaLinear = ImageDisk<{ ImageType::RgbaLinear as u32 }>;
pub type ImageDiskRgbaSrgb = ImageDisk<{ ImageType::RgbaColor as u32 }>;

pub trait ImageDiskTrait {
	const IMAGE_TYPE: ImageType;

	fn new(id: usize) -> Self;

	fn id(&self) -> usize;
}

impl<const IMAGE_TYPE: u32> ImageDiskTrait for ImageDisk<IMAGE_TYPE> {
	const IMAGE_TYPE: ImageType = ImageType::from_u32(IMAGE_TYPE);

	fn new(id: usize) -> Self {
		Self { id }
	}

	fn id(&self) -> usize {
		self.id
	}
}

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct ImageStorage {
	pub images: Vec<(DynImage<'static>, String)>,
}
