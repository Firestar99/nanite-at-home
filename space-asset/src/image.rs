#[cfg(feature = "disk")]
mod disk {
	use rkyv::{Archive, Deserialize, Serialize};
	use std::error::Error;
	use std::fmt::{Display, Formatter};

	mod image_encoding {
		#![allow(non_camel_case_types)]

		use rkyv::{Archive, Deserialize, Serialize};

		#[repr(u8)]
		#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
		pub enum ImageEncoding {
			R_UNORM,
			RG_UNORM,
			RGBA_UNORM,
			RGBA_SRGB,
			/// grayscale
			BC4_UNORM,
			/// tangent space normal maps
			BC5_UNORM,
			/// high quality rgba image
			BC7_UNORM,
			/// high quality rgba image
			BC7_SRGB,
		}
	}
	pub use image_encoding::*;

	impl ImageEncoding {
		pub fn channels(&self) -> u32 {
			match self {
				ImageEncoding::R_UNORM => 1,
				ImageEncoding::RG_UNORM => 2,
				ImageEncoding::RGBA_UNORM => 4,
				ImageEncoding::RGBA_SRGB => 4,
				ImageEncoding::BC4_UNORM => 1,
				ImageEncoding::BC5_UNORM => 2,
				ImageEncoding::BC7_UNORM => 4,
				ImageEncoding::BC7_SRGB => 4,
			}
		}

		pub fn block_size(&self) -> Size {
			match self {
				ImageEncoding::R_UNORM
				| ImageEncoding::RG_UNORM
				| ImageEncoding::RGBA_UNORM
				| ImageEncoding::RGBA_SRGB => Size::new(1, 1),
				ImageEncoding::BC4_UNORM
				| ImageEncoding::BC5_UNORM
				| ImageEncoding::BC7_UNORM
				| ImageEncoding::BC7_SRGB => Size::new(4, 4),
			}
		}

		pub fn bytes_per_block(&self) -> usize {
			match self {
				ImageEncoding::R_UNORM => 1,
				ImageEncoding::RG_UNORM => 2,
				ImageEncoding::RGBA_UNORM => 4,
				ImageEncoding::RGBA_SRGB => 4,
				ImageEncoding::BC4_UNORM => 8,
				ImageEncoding::BC5_UNORM => 16,
				ImageEncoding::BC7_UNORM => 16,
				ImageEncoding::BC7_SRGB => 16,
			}
		}
	}

	#[repr(C)]
	#[derive(Copy, Clone, Default, Debug, Archive, Serialize, Deserialize)]
	pub struct Size {
		pub width: u32,
		pub height: u32,
	}

	impl Size {
		pub fn new(width: u32, height: u32) -> Self {
			Self { width, height }
		}
	}

	#[repr(C)]
	#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct Image2DMetadata {
		pub size: Size,
		pub encoding: ImageEncoding,
	}

	#[repr(C)]
	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct Image2DDisk {
		pub bytes: Box<[u8]>,
		pub metadata: Image2DMetadata,
	}

	#[derive(Clone, Debug)]
	pub enum ImageValidationError {
		UnalignedSize { encoding: ImageEncoding, size: Size },
		BytesLenMismatch { expected: usize, actual: usize },
	}

	impl Display for ImageValidationError {
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			match self {
				ImageValidationError::UnalignedSize { encoding, size } => {
					write!(
						f,
						"Encoding {:?} expects a block size of {:?} but image size {:?} was misaligned",
						encoding,
						encoding.block_size(),
						size
					)
				}
				ImageValidationError::BytesLenMismatch { expected, actual } => {
					write!(f, "Expected image to be {} bytes but was {} bytes", expected, actual)
				}
			}
		}
	}

	impl Error for ImageValidationError {}

	impl Image2DMetadata {
		pub(crate) fn validate(&self, bytes_len: usize) -> Result<(), ImageValidationError> {
			let block_size = self.encoding.block_size();
			if !(self.size.width % block_size.width == 0 && self.size.height % block_size.height == 0) {
				return Err(ImageValidationError::UnalignedSize {
					encoding: self.encoding,
					size: self.size,
				});
			}
			let block_size = Size::new(self.size.width / block_size.width, self.size.height / block_size.height);
			let bytes_expected =
				block_size.width as usize * block_size.height as usize * self.encoding.bytes_per_block();
			if bytes_expected != bytes_len {
				return Err(ImageValidationError::BytesLenMismatch {
					expected: bytes_expected,
					actual: bytes_len,
				});
			}
			Ok(())
		}
	}

	impl Image2DDisk {
		pub fn validate(&self) -> Result<(), ImageValidationError> {
			self.metadata.validate(self.bytes.len())
		}
	}
}

#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::image::{ArchivedImage2DDisk, ImageEncoding};
	use crate::uploader::{UploadError, Uploader};
	use vulkano::format::Format;
	use vulkano::Validated;
	use vulkano_bindless::descriptor::RC;
	use vulkano_bindless::spirv_std::image::Image2d;
	use vulkano_bindless_shaders::descriptor::Desc;

	impl ImageEncoding {
		pub fn vulkano_format(&self) -> Format {
			match *self {
				ImageEncoding::R_UNORM => Format::R8_UNORM,
				ImageEncoding::RG_UNORM => Format::R8G8_UNORM,
				ImageEncoding::RGBA_UNORM => Format::R8G8B8A8_UNORM,
				ImageEncoding::RGBA_SRGB => Format::R8G8B8A8_SRGB,
				ImageEncoding::BC4_UNORM => Format::BC4_UNORM_BLOCK,
				ImageEncoding::BC5_UNORM => Format::BC5_UNORM_BLOCK,
				ImageEncoding::BC7_UNORM => Format::BC7_UNORM_BLOCK,
				ImageEncoding::BC7_SRGB => Format::BC7_SRGB_BLOCK,
			}
		}
	}

	impl ArchivedImage2DDisk {
		pub async fn upload(&self, uploader: &Uploader) -> Result<Desc<RC, Image2d>, Validated<UploadError>> {
			Ok(uploader.upload_image2d(self).await?.into())
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
