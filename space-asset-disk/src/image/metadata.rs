#![allow(non_camel_case_types)]

use crate::image::Size;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rkyv::{Archive, Deserialize, Infallible, Serialize};

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Archive, Serialize, Deserialize)]
pub enum ImageType {
	/// Single channel of values.
	/// Usually represented by R8 or BC4.
	R_VALUES,
	/// Two channels of values which are compressed independently. Optimal for normal maps.
	/// Usually represented by RG8 or BC5.
	RG_VALUES,
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
			ImageType::R_VALUES,
			ImageType::RG_VALUES,
			ImageType::RGBA_LINEAR,
			ImageType::RGBA_COLOR,
		][value as usize]
	}

	pub const fn channels(&self) -> u32 {
		match self {
			ImageType::R_VALUES => 1,
			ImageType::RG_VALUES => 2,
			ImageType::RGBA_LINEAR => 4,
			ImageType::RGBA_COLOR => 4,
		}
	}
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
pub enum DiskImageCompression {
	None,
	BCn_zstd,
	Embedded,
}

impl DiskImageCompression {
	pub const fn decodes_into(&self) -> RuntimeImageCompression {
		match self {
			DiskImageCompression::None => RuntimeImageCompression::None,
			DiskImageCompression::BCn_zstd => RuntimeImageCompression::BCn,
			DiskImageCompression::Embedded => RuntimeImageCompression::None,
		}
	}
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
pub enum RuntimeImageCompression {
	None,
	BCn,
}

impl RuntimeImageCompression {
	pub const fn block_size(&self) -> Size {
		match self {
			RuntimeImageCompression::None => Size::new(1, 1),
			RuntimeImageCompression::BCn => Size::new(4, 4),
		}
	}

	pub const fn bytes_per_block(&self, data_type: ImageType) -> usize {
		match self {
			RuntimeImageCompression::None => data_type.channels() as usize,
			RuntimeImageCompression::BCn => {
				match data_type {
					// BC4
					ImageType::R_VALUES => 8,
					// BC5
					ImageType::RG_VALUES => 16,
					// BC7
					ImageType::RGBA_LINEAR => 16,
					ImageType::RGBA_COLOR => 16,
				}
			}
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct Image2DMetadata<const IMAGE_TYPE: u32> {
	pub disk_compression: DiskImageCompression,
	pub size: Size,
}

impl<const IMAGE_TYPE: u32> ArchivedImage2DMetadata<IMAGE_TYPE> {
	pub fn deserialize(&self) -> Image2DMetadata<IMAGE_TYPE> {
		Deserialize::deserialize(self, &mut Infallible).unwrap()
	}
}

impl<const IMAGE_TYPE: u32> Image2DMetadata<IMAGE_TYPE> {
	pub const fn image_type(&self) -> ImageType {
		ImageType::try_from_const(IMAGE_TYPE)
	}

	pub fn runtime_compression(&self) -> RuntimeImageCompression {
		self.disk_compression.decodes_into()
	}

	pub fn decompressed_bytes(&self) -> usize {
		let runtime = self.runtime_compression();
		let block_size = runtime.block_size();
		let (width, height) = if block_size != Size::new(1, 1) {
			assert!(
				self.size.width % block_size.width == 0 && self.size.height % block_size.height == 0,
				"Image size {:?} was not dividable by block size {:?}",
				self.size,
				block_size
			);
			(self.size.width / block_size.width, self.size.height / block_size.height)
		} else {
			(self.size.width, self.size.height)
		};
		width as usize * height as usize * runtime.bytes_per_block(self.image_type())
	}
}
