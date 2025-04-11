use crate::image::{ImageType, RuntimeImage, RuntimeImageMetadata};
use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Debug, Display};

/// Some Image that can be stored on disk
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct DiskImage<'a, const IMAGE_TYPE: u32> {
	meta: DiskImageMetadata<IMAGE_TYPE>,
	data: Cow<'a, [u8]>,
}

pub type DiskImageRLinear<'a> = DiskImage<'a, { ImageType::R_LINEAR as u32 }>;
pub type DiskImageRgLinear<'a> = DiskImage<'a, { ImageType::RG_VALUES as u32 }>;
pub type DiskImageRgbaLinear<'a> = DiskImage<'a, { ImageType::RGBA_LINEAR as u32 }>;
pub type DiskImageRgbaSrgb<'a> = DiskImage<'a, { ImageType::RGBA_COLOR as u32 }>;

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub enum DiskImageMetadata<const IMAGE_TYPE: u32> {
	Uncompressed(UncompressedImageMetadata<IMAGE_TYPE>),
	BCn(BCnImageMetadata<IMAGE_TYPE>),
	ZstdBCn(ZstdBCnImageMetadata<IMAGE_TYPE>),
	// Embedded(EmbeddedMetadata<IMAGE_TYPE>),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct UncompressedImageMetadata<const IMAGE_TYPE: u32> {
	pub extent: UVec3,
	pub mip_layers: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct BCnImageMetadata<const IMAGE_TYPE: u32> {
	block_count: UVec3,
	mip_layers: u32,
}

impl<const IMAGE_TYPE: u32> BCnImageMetadata<IMAGE_TYPE> {
	pub fn image_type() -> ImageType {
		ImageType::try_from(IMAGE_TYPE).unwrap()
	}

	pub fn bytes_per_block(&self) -> u32 {
		match Self::image_type() {
			// BC4
			ImageType::R_LINEAR => 8,
			// BC5
			ImageType::RG_VALUES => 16,
			// BC7
			ImageType::RGBA_LINEAR => 16,
			ImageType::RGBA_COLOR => 16,
		}
	}
}

#[derive(Clone, Debug)]
pub struct ZstdBCnImageMetadata<const IMAGE_TYPE: u32> {
	pub inner: BCnImageMetadata<IMAGE_TYPE>,
}

/// An Image that can be decoded to [`RuntimeImage`]. Usually used as a disk format that needs to be
/// decompressed before it's usable by the GPU.
pub trait DecodeToRuntimeImage {
	fn decoded_metadata(&self) -> RuntimeImageMetadata;

	fn decode_into(&self, src: &[u8], dst: &mut [u8]);

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		let meta = self.decoded_metadata();
		let mut dst = vec![0; meta.size_of_total()];
		self.decode_into(src, &mut dst);
		RuntimeImage {
			meta,
			data: Cow::Owned(dst),
		}
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for UncompressedImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		let image_type = ImageType::try_from(IMAGE_TYPE).unwrap();
		RuntimeImageMetadata::new_uncompressed(image_type, self.extent, image_type.channels(), self.mip_layers).unwrap()
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		dst.copy_from_slice(src)
	}

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		RuntimeImage {
			meta: self.decoded_metadata(),
			data: Cow::Borrowed(src),
		}
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for ZstdBCnImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		self.inner.decoded_metadata()
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		zstd::stream::copy_decode(src, dst).unwrap();
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for BCnImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new(
			ImageType::try_from(IMAGE_TYPE).unwrap(),
			UVec3::new(4, 4, 1),
			self.block_count,
			self.bytes_per_block(),
			self.mip_layers,
		)
		.unwrap()
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		dst.copy_from_slice(src)
	}

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		RuntimeImage {
			meta: self.decoded_metadata(),
			data: Cow::Borrowed(src),
		}
	}
}
