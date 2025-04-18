use crate::image::{ImageType, RuntimeImage, RuntimeImageCompression, RuntimeImageMetadata};
use glam::UVec3;
use rkyv::api::high::HighDeserializer;
use rkyv::rancor::Panic;
use rkyv::with::AsOwned;
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;

/// Some Image that can be stored on disk
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct Image<'a, M> {
	pub meta: M,
	#[rkyv(with = AsOwned)]
	pub data: Cow<'a, [u8]>,
}

impl<M> Image<'_, M> {
	pub fn into_owned(self) -> Image<'static, M> {
		profiling::function_scope!();
		Image {
			meta: self.meta,
			data: Cow::Owned(self.data.into_owned()),
		}
	}
}

impl<M> ArchivedImage<'_, M>
where
	M: Archive,
	M::Archived: Deserialize<M, HighDeserializer<Panic>>,
{
	pub fn to_image(&self) -> Image<'_, M> {
		Image {
			meta: rkyv::deserialize(&self.meta).unwrap(),
			data: Cow::Borrowed(&self.data),
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

impl<'a, M: ToDynImage> Image<'a, M> {
	pub fn to_dyn_image(&self) -> DynImage {
		DynImage {
			meta: self.meta.to_dyn_image(),
			data: Cow::Borrowed(&self.data),
		}
	}

	pub fn into_dyn_image(self) -> DynImage<'a> {
		DynImage {
			meta: self.meta.to_dyn_image(),
			data: self.data,
		}
	}
}

pub trait ToDynImage {
	fn to_dyn_image(&self) -> DynImageMetadata;
}

pub type DynImage<'a> = Image<'a, DynImageMetadata>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub enum DynImageMetadata {
	Uncompressed(UncompressedImageMetadata),
	BCn(BCnImageMetadata),
	ZstdBCn(ZstdBCnImageMetadata),
	Embedded(EmbeddedImageMetadata),
	SinglePixel(SinglePixelMetadata),
}

#[cold]
#[cfg(not(feature = "image_decoding"))]
fn missing_image_decoding_feature() -> ! {
	panic!("Missing feature \"image_decoding\" to decode an embedded image");
}

impl DecodeToRuntimeImage for DynImageMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		match self {
			DynImageMetadata::Uncompressed(m) => m.decoded_metadata(),
			DynImageMetadata::BCn(m) => m.decoded_metadata(),
			DynImageMetadata::ZstdBCn(m) => m.decoded_metadata(),
			#[cfg(feature = "image_decoding")]
			DynImageMetadata::Embedded(m) => m.decoded_metadata(),
			#[cfg(not(feature = "image_decoding"))]
			DynImageMetadata::Embedded(_) => missing_image_decoding_feature(),
			DynImageMetadata::SinglePixel(m) => m.decoded_metadata(),
		}
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		match self {
			DynImageMetadata::Uncompressed(m) => m.decode_into(src, dst),
			DynImageMetadata::BCn(m) => m.decode_into(src, dst),
			DynImageMetadata::ZstdBCn(m) => m.decode_into(src, dst),
			#[cfg(feature = "image_decoding")]
			DynImageMetadata::Embedded(m) => m.decode_into(src, dst),
			#[cfg(not(feature = "image_decoding"))]
			DynImageMetadata::Embedded(_) => missing_image_decoding_feature(),
			DynImageMetadata::SinglePixel(m) => m.decode_into(src, dst),
		}
	}

	fn decode<'a>(&self, src: &'a [u8]) -> RuntimeImage<'a> {
		match self {
			DynImageMetadata::Uncompressed(m) => m.decode(src),
			DynImageMetadata::BCn(m) => m.decode(src),
			DynImageMetadata::ZstdBCn(m) => m.decode(src),
			#[cfg(feature = "image_decoding")]
			DynImageMetadata::Embedded(m) => m.decode(src),
			#[cfg(not(feature = "image_decoding"))]
			DynImageMetadata::Embedded(_) => missing_image_decoding_feature(),
			DynImageMetadata::SinglePixel(m) => m.decode(src),
		}
	}
}

pub type UncompressedImage<'a> = Image<'a, UncompressedImageMetadata>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct UncompressedImageMetadata {
	pub image_type: ImageType,
	pub extent: UVec3,
	pub mip_layers: u32,
}

impl ToDynImage for UncompressedImageMetadata {
	fn to_dyn_image(&self) -> DynImageMetadata {
		DynImageMetadata::Uncompressed(*self)
	}
}

impl DecodeToRuntimeImage for UncompressedImageMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new_uncompressed(
			self.image_type,
			self.extent,
			self.image_type.channels(),
			self.mip_layers,
		)
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

pub type BCnImage<'a> = Image<'a, BCnImageMetadata>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct BCnImageMetadata {
	pub image_type: ImageType,
	pub extent: UVec3,
	pub mip_layers: u32,
}

impl ToDynImage for BCnImageMetadata {
	fn to_dyn_image(&self) -> DynImageMetadata {
		DynImageMetadata::BCn(*self)
	}
}

impl BCnImageMetadata {
	pub const BLOCK_SIZE: UVec3 = UVec3::new(4, 4, 1);

	pub fn bytes_per_block(&self) -> u32 {
		match self.image_type {
			// BC4
			ImageType::RValue => 8,
			// BC5
			ImageType::RgValue => 16,
			// BC7
			ImageType::RgbaLinear => 16,
			ImageType::RgbaColor => 16,
		}
	}
}

impl DecodeToRuntimeImage for BCnImageMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new(
			self.image_type,
			RuntimeImageCompression::BCn,
			self.extent,
			Self::BLOCK_SIZE,
			self.bytes_per_block(),
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

impl BCnImage<'_> {
	pub fn compress_to_zstd(&self, zstd_level: i32) -> ZstdBCnImage<'static> {
		profiling::function_scope!();
		ZstdBCnImage {
			meta: ZstdBCnImageMetadata { inner: self.meta },
			data: Cow::Owned(zstd::stream::encode_all(self.data.as_ref(), zstd_level).unwrap()),
		}
	}
}

pub type ZstdBCnImage<'a> = Image<'a, ZstdBCnImageMetadata>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct ZstdBCnImageMetadata {
	pub inner: BCnImageMetadata,
}

impl ToDynImage for ZstdBCnImageMetadata {
	fn to_dyn_image(&self) -> DynImageMetadata {
		DynImageMetadata::ZstdBCn(*self)
	}
}

impl DecodeToRuntimeImage for ZstdBCnImageMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		self.inner.decoded_metadata()
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		profiling::function_scope!();
		zstd::stream::copy_decode(src, dst).unwrap();
	}
}

impl ZstdBCnImage<'_> {
	pub fn decompress_to_bcn(&self) -> BCnImage<'static> {
		profiling::function_scope!();
		BCnImage {
			meta: self.meta.inner,
			data: self.meta.decode(self.data.as_ref()).data,
		}
		// we already own it, doesn't clone anything
		.into_owned()
	}
}

// Embedded image file, like png or jpg
pub type EmbeddedImage<'a> = Image<'a, EmbeddedImageMetadata>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct EmbeddedImageMetadata {
	pub image_type: ImageType,
	pub extent: UVec3,
}

impl ToDynImage for EmbeddedImageMetadata {
	fn to_dyn_image(&self) -> DynImageMetadata {
		DynImageMetadata::Embedded(*self)
	}
}

// Embedded image file, like png or jpg
pub type SinglePixelImage<'a> = Image<'a, SinglePixelMetadata>;

#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct SinglePixelMetadata {
	pub image_type: ImageType,
	pub color: [u8; 4],
}

impl SinglePixelMetadata {
	pub fn new(image_type: ImageType, color: [u8; 4]) -> Self {
		Self { image_type, color }
	}
	pub fn new_r_values(color: u8) -> Self {
		Self::new(ImageType::RgValue, [color, 1, 1, 1])
	}
	pub fn new_rg_values(color: [u8; 2]) -> Self {
		Self::new(ImageType::RgValue, [color[0], color[1], 1, 1])
	}
	pub fn new_rgba_linear(color: [u8; 4]) -> Self {
		Self::new(ImageType::RgbaLinear, color)
	}
	pub fn new_rgba_srgb(color: [u8; 4]) -> Self {
		Self::new(ImageType::RgbaColor, color)
	}
	pub fn to_image(&self) -> SinglePixelImage {
		SinglePixelImage {
			meta: *self,
			data: Cow::Borrowed(&[]),
		}
	}
}

impl ToDynImage for SinglePixelMetadata {
	fn to_dyn_image(&self) -> DynImageMetadata {
		DynImageMetadata::SinglePixel(*self)
	}
}

impl DecodeToRuntimeImage for SinglePixelMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new_uncompressed(self.image_type, UVec3::ONE, self.image_type.channels(), 1)
	}

	fn decode_into(&self, _src: &[u8], dst: &mut [u8]) {
		dst.copy_from_slice(&self.color[0..dst.len()])
	}
}
