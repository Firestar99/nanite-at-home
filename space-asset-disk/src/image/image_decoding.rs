use crate::image::{
	DecodeToRuntimeImage, EmbeddedImage, EmbeddedImageMetadata, RuntimeImageMetadata, UncompressedImage,
	UncompressedImageMetadata, ZstdBCnImage,
};
use glam::UVec3;
use image::{ImageReader, ImageResult};
use std::borrow::Cow;

impl<const IMAGE_TYPE: u32> EmbeddedImage<IMAGE_TYPE> {
	pub fn new(src: &[u8]) -> ImageResult<Self> {
		let decoder = ImageReader::new(src).with_guessed_format()?.into_decoder()?;
		let dim = decoder.dimensions();
		Ok(Self {
			meta: EmbeddedImageMetadata {
				extent: UVec3::new(dim.0, dim.1, 1),
			},
			data: Cow::Borrowed(src),
		})
	}
}

impl<const IMAGE_TYPE: u32> DecodeToRuntimeImage for EmbeddedImageMetadata<IMAGE_TYPE> {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		let image_type = Self::image_type();
		RuntimeImageMetadata::new_uncompressed(image_type, self.extent, image_type.channels(), 1)
	}

	fn decode_into(&self, src: &[u8], dst: &mut [u8]) {
		profiling::function_scope!();
		let decoder = ImageReader::new(src)
			.with_guessed_format()
			.unwrap()
			.into_decoder()
			.unwrap();
		decoder.read_image(dst).unwrap();
	}
}

impl<const IMAGE_TYPE: u32> EmbeddedImage<'_, IMAGE_TYPE> {
	pub fn decode_to_uncompressed(&self) -> UncompressedImage<'static, IMAGE_TYPE> {
		profiling::function_scope!();
		UncompressedImage {
			meta: UncompressedImageMetadata { extent, mip_layers: 1 },
			data: self.meta.decode(self.data.as_ref()).data,
		}
		// we already own it, doesn't clone anything
		.into_owned()
	}
}
