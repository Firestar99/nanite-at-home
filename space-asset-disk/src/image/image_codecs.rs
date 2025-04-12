use crate::image::{ImageType, RuntimeImage, RuntimeImageMetadata};
use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;

/// Some Image that can be stored on disk
#[derive(Clone, Debug)]
pub struct Image<'a, M> {
	pub meta: M,
	pub data: Cow<'a, [u8]>,
}

impl<M> Image<'_, M> {
	pub fn into_owned(self) -> Image<'static, M> {
		Image {
			meta: self.meta,
			data: Cow::Owned(self.data.into_owned()),
		}
	}
}

impl<M: DecodeToRuntimeImage> Image<'_, M> {
	pub fn decoded_metadata(&self) -> RuntimeImageMetadata {
		self.meta.decoded_metadata()
	}

	pub fn decode_into(&self, dst: &mut [u8]) {
		self.meta.decode_into(&self.data, dst)
	}

	pub fn decode(&self) -> RuntimeImage {
		self.meta.decode(&self.data)
	}
}

/// An Image that can be decoded to [`RuntimeImage`]. Usually used as a disk format that needs to be
/// decompressed before it's usable by the GPU.
pub trait DecodeToRuntimeImage {
	fn decoded_metadata(&self) -> RuntimeImageMetadata;

	fn decode_into(&self, src: &[u8], dst: &mut [u8]);

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		let meta = self.decoded_metadata();
		let mut dst = vec![0; meta.total_size];
		self.decode_into(src, &mut dst);
		RuntimeImage {
			meta,
			data: Cow::Owned(dst),
		}
	}
}

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub enum DiskImageMetadata<const IMAGE_TYPE: u32> {
	Uncompressed(UncompressedImageMetadata<IMAGE_TYPE>),
	BCn(BCnImageMetadata<IMAGE_TYPE>),
	ZstdBCn(ZstdBCnImageMetadata<IMAGE_TYPE>),
	Embedded(EmbeddedImageMetadata<IMAGE_TYPE>),
}

#[cold]
fn missing_image_decoding_feature() -> ! {
	panic!("Missing feature \"image_decoding\" to decode an embedded image");
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for DiskImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		match self {
			DiskImageMetadata::Uncompressed(m) => m.decoded_metadata(),
			DiskImageMetadata::BCn(m) => m.decoded_metadata(),
			DiskImageMetadata::ZstdBCn(m) => m.decoded_metadata(),
			#[cfg(feature = "image_decoding")]
			DiskImageMetadata::Embedded(m) => m.decoded_metadata(),
			#[cfg(not(feature = "image_decoding"))]
			DiskImageMetadata::Embedded(_) => missing_image_decoding_feature(),
		}
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		match self {
			DiskImageMetadata::Uncompressed(m) => m.decode_into(src, dst),
			DiskImageMetadata::BCn(m) => m.decode_into(src, dst),
			DiskImageMetadata::ZstdBCn(m) => m.decode_into(src, dst),
			#[cfg(feature = "image_decoding")]
			DiskImageMetadata::Embedded(m) => m.decode_into(src, dst),
			#[cfg(not(feature = "image_decoding"))]
			DiskImageMetadata::Embedded(_) => missing_image_decoding_feature(),
		}
	}

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		match self {
			DiskImageMetadata::Uncompressed(m) => m.decode(src),
			DiskImageMetadata::BCn(m) => m.decode(src),
			DiskImageMetadata::ZstdBCn(m) => m.decode(src),
			#[cfg(feature = "image_decoding")]
			DiskImageMetadata::Embedded(m) => m.decode(src),
			#[cfg(not(feature = "image_decoding"))]
			DiskImageMetadata::Embedded(_) => missing_image_decoding_feature(),
		}
	}
}

pub type UncompressedImage<'a, const IMAGE_TYPE: u32> = Image<'a, UncompressedImageMetadata<IMAGE_TYPE>>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct UncompressedImageMetadata<const IMAGE_TYPE: u32> {
	pub extent: UVec3,
	pub mip_layers: u32,
}

impl<const IMAGE_TYPE: u32> UncompressedImageMetadata<IMAGE_TYPE> {
	pub fn image_type() -> ImageType {
		ImageType::try_from(IMAGE_TYPE).unwrap()
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for UncompressedImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		let image_type = Self::image_type();
		RuntimeImageMetadata::new_uncompressed(image_type, self.extent, image_type.channels(), self.mip_layers)
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

pub type BCnImage<'a, const IMAGE_TYPE: u32> = Image<'a, BCnImageMetadata<IMAGE_TYPE>>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct BCnImageMetadata<const IMAGE_TYPE: u32> {
	pub extent: UVec3,
	pub mip_layers: u32,
}

impl<const IMAGE_TYPE: u32> BCnImageMetadata<IMAGE_TYPE> {
	pub fn image_type() -> ImageType {
		ImageType::try_from(IMAGE_TYPE).unwrap()
	}

	pub fn block_size() -> UVec3 {
		UVec3::new(4, 4, 1)
	}

	pub fn bytes_per_block() -> u32 {
		match Self::image_type() {
			// BC4
			ImageType::R_VALUE => 8,
			// BC5
			ImageType::RG_VALUE => 16,
			// BC7
			ImageType::RGBA_LINEAR => 16,
			ImageType::RGBA_COLOR => 16,
		}
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for BCnImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new(
			ImageType::try_from(IMAGE_TYPE).unwrap(),
			self.extent,
			Self::block_size(),
			Self::bytes_per_block(),
			self.mip_layers,
		)
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		profiling::function_scope!();
		dst.copy_from_slice(src)
	}

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		RuntimeImage {
			meta: self.decoded_metadata(),
			data: Cow::Borrowed(src),
		}
	}
}

impl<const IMAGE_TYPE: u32> BCnImage<'_, IMAGE_TYPE> {
	pub fn compress_to_zstd(&self, zstd_level: i32) -> ZstdBCnImage<'static, IMAGE_TYPE> {
		profiling::function_scope!();
		ZstdBCnImage {
			meta: ZstdBCnImageMetadata { inner: self.meta },
			data: Cow::Owned(zstd::stream::encode_all(self.data.as_ref(), zstd_level).unwrap()),
		}
	}
}

pub type ZstdBCnImage<'a, const IMAGE_TYPE: u32> = Image<'a, ZstdBCnImageMetadata<IMAGE_TYPE>>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct ZstdBCnImageMetadata<const IMAGE_TYPE: u32> {
	pub inner: BCnImageMetadata<IMAGE_TYPE>,
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for ZstdBCnImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		self.inner.decoded_metadata()
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		profiling::function_scope!();
		zstd::stream::copy_decode(src, dst).unwrap();
	}
}

impl<const IMAGE_TYPE: u32> ZstdBCnImage<'_, IMAGE_TYPE> {
	pub fn decompress_to_bcn(&self) -> BCnImage<'static, IMAGE_TYPE> {
		profiling::function_scope!();
		BCnImage {
			meta: self.meta.inner,
			data: self.meta.decode(self.data.as_ref()).data,
		}
		// we already own it, doesn't clone anything
		.into_owned()
	}
}

pub type EmbeddedImage<'a, const IMAGE_TYPE: u32> = Image<'a, EmbeddedImageMetadata<IMAGE_TYPE>>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct EmbeddedImageMetadata<const IMAGE_TYPE: u32> {
	pub extent: UVec3,
}

impl<const IMAGE_TYPE: u32> EmbeddedImageMetadata<IMAGE_TYPE> {
	pub fn image_type() -> ImageType {
		ImageType::try_from(IMAGE_TYPE).unwrap()
	}
}
