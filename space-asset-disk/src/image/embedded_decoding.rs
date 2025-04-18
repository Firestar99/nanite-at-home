use crate::image::{
	DecodeToRuntimeImage, EmbeddedImage, EmbeddedImageMetadata, ImageType, RuntimeImageMetadata, UncompressedImage,
	UncompressedImageMetadata,
};
use glam::UVec3;
use image::{ImageReader, ImageResult};
use std::borrow::Cow;

impl EmbeddedImage<'_> {
	pub fn new(image_type: ImageType, src: &[u8]) -> ImageResult<Self> {
		let decoder = ImageReader::new(src).with_guessed_format()?.into_decoder()?;
		let dim = decoder.dimensions();
		Ok(Self {
			meta: EmbeddedImageMetadata {
				image_type,
				extent: UVec3::new(dim.0, dim.1, 1),
			},
			data: Cow::Borrowed(src),
		})
	}

	pub fn decode_to_uncompressed(&self) -> UncompressedImage<'static> {
		profiling::function_scope!();
		UncompressedImage {
			meta: UncompressedImageMetadata {
				image_type: self.meta.image_type,
				extent: self.meta.extent,
				mip_layers: 1,
			},
			data: self.meta.decode(self.data.as_ref()).data,
		}
		// we already own it, doesn't clone anything
		.into_owned()
	}
}

impl DecodeToRuntimeImage for EmbeddedImageMetadata {
	fn decoded_metadata(&self) -> RuntimeImageMetadata {
		RuntimeImageMetadata::new_uncompressed(self.image_type, self.extent, self.image_type.channels(), 1)
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
