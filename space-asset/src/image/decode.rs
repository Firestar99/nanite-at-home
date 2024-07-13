use crate::image::{ArchivedImage2DDisk, DiskImageCompression, Image2DDisk, Image2DMetadata};
use std::io;
use zune_image::codecs::png::zune_core::bytestream::ZCursor;
use zune_image::codecs::png::zune_core::colorspace::ColorSpace;
use zune_image::codecs::png::zune_core::options::DecoderOptions;
use zune_image::errors::ImageErrors;

impl<const DATA_TYPE: u32> Image2DDisk<DATA_TYPE> {
	pub fn decode(&self) -> Result<Vec<u8>, ImageErrors> {
		self.metadata.decode(&self.bytes)
	}

	pub fn decode_into(&self, dst: &mut [u8]) -> Result<(), ImageErrors> {
		self.metadata.decode_into(&self.bytes, dst)
	}
}

impl<const DATA_TYPE: u32> ArchivedImage2DDisk<DATA_TYPE> {
	pub fn decode(&self) -> Result<Vec<u8>, ImageErrors> {
		self.metadata.deserialize().decode(&self.bytes)
	}

	pub fn decode_into(&self, dst: &mut [u8]) -> Result<(), ImageErrors> {
		self.metadata.deserialize().decode_into(&self.bytes, dst)
	}
}

impl<const DATA_TYPE: u32> Image2DMetadata<DATA_TYPE> {
	pub(super) fn decode(&self, src: &[u8]) -> Result<Vec<u8>, ImageErrors> {
		let mut vec = vec![0; self.decompressed_bytes()];
		self.decode_into(src, &mut *vec)?;
		Ok(vec)
	}

	#[profiling::function]
	pub(super) fn decode_into(&self, src: &[u8], dst: &mut [u8]) -> Result<(), ImageErrors> {
		assert_eq!(dst.len(), self.decompressed_bytes());
		match self.disk_compression {
			DiskImageCompression::None => self.decode_none_into(src, dst),
			DiskImageCompression::BCn_zstd => self.decode_bcn_zstd_into(src, dst)?,
			DiskImageCompression::Embedded => self.decode_embedded_into(src, dst)?,
		}
		Ok(())
	}

	#[profiling::function]
	fn decode_none_into(&self, src: &[u8], dst: &mut [u8]) {
		assert_eq!(dst.len(), src.len());
		dst.copy_from_slice(src);
	}

	#[profiling::function]
	fn decode_bcn_zstd_into(&self, src: &[u8], dst: &mut [u8]) -> io::Result<()> {
		let written = zstd::bulk::decompress_to_buffer(src, dst)?;
		assert_eq!(written, dst.len());
		Ok(())
	}

	#[profiling::function]
	fn decode_embedded_into(&self, src: &[u8], dst: &mut [u8]) -> Result<(), ImageErrors> {
		let mut image = zune_image::image::Image::read(ZCursor::new(src), DecoderOptions::new_fast())?;
		assert_eq!(image.frames_len(), 1);
		image.convert_color(ColorSpace::RGBA)?;
		image.frames_ref()[0].flatten_into(dst)?;
		Ok(())
	}
}
