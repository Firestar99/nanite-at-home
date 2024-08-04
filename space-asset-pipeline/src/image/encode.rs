use crate::gltf::GltfImageError;
use intel_tex_2::{bc4, bc5, bc7, RSurface, RgSurface, RgbaSurface};
use space_asset::image::{
	DiskImageCompression, Image2DDisk, Image2DMetadata, ImageType, RuntimeImageCompression, Size,
};

#[derive(Copy, Clone, Debug)]
pub struct Bc7Settings {
	opaque: bc7::EncodeSettings,
	alpha: bc7::EncodeSettings,
}

#[derive(Copy, Clone, Debug)]
pub struct EncodeSettings {
	bc4_bc5: bool,
	bc7: Option<Bc7Settings>,
	zstd_level: i32,
}

impl EncodeSettings {
	pub fn embedded() -> Self {
		Self {
			bc4_bc5: false,
			bc7: None,
			zstd_level: 1,
		}
	}

	pub fn ultra_fast() -> Self {
		Self {
			bc4_bc5: true,
			bc7: Some(Bc7Settings {
				opaque: bc7::opaque_ultra_fast_settings(),
				alpha: bc7::alpha_ultra_fast_settings(),
			}),
			zstd_level: 3,
		}
	}

	pub fn very_fast() -> Self {
		Self {
			bc4_bc5: true,
			bc7: Some(Bc7Settings {
				opaque: bc7::opaque_very_fast_settings(),
				alpha: bc7::alpha_very_fast_settings(),
			}),
			zstd_level: 3,
		}
	}

	pub fn fast() -> Self {
		Self {
			bc4_bc5: true,
			bc7: Some(Bc7Settings {
				opaque: bc7::opaque_fast_settings(),
				alpha: bc7::alpha_fast_settings(),
			}),
			zstd_level: 3,
		}
	}

	pub fn basic() -> Self {
		Self {
			bc4_bc5: true,
			bc7: Some(Bc7Settings {
				opaque: bc7::opaque_basic_settings(),
				alpha: bc7::alpha_basic_settings(),
			}),
			zstd_level: 3,
		}
	}

	pub fn slow() -> Self {
		Self {
			bc4_bc5: true,
			bc7: Some(Bc7Settings {
				opaque: bc7::opaque_slow_settings(),
				alpha: bc7::alpha_slow_settings(),
			}),
			zstd_level: 5,
		}
	}
}

impl Default for EncodeSettings {
	fn default() -> Self {
		Self::ultra_fast()
	}
}

pub trait Encode: Sized {
	fn into_optimal_encode(self, settings: EncodeSettings) -> Result<Self, GltfImageError> {
		match self.to_optimal_encode(settings) {
			Ok(Some(e)) => Ok(e),
			Ok(None) => Ok(self),
			Err(err) => Err(err),
		}
	}

	fn to_optimal_encode(&self, settings: EncodeSettings) -> Result<Option<Self>, GltfImageError>;

	fn to_none_encode(&self, settings: EncodeSettings) -> Result<Self, GltfImageError>;

	fn to_bc_encode(&self, settings: EncodeSettings) -> Result<Self, GltfImageError>;
}

impl<const IMAGE_TYPE: u32> Encode for Image2DDisk<IMAGE_TYPE> {
	#[profiling::function]
	fn to_optimal_encode(&self, settings: EncodeSettings) -> Result<Option<Self>, GltfImageError> {
		if self.metadata.runtime_compression() != RuntimeImageCompression::None {
			return Ok(None);
		}
		let size = self.metadata.size;
		if (size.width * size.height) < 1024 {
			Ok(Some(self.to_none_encode(settings)?))
		} else if size.width > 4 && size.height > 4 {
			match self.to_bc_encode(settings) {
				Ok(e) => Ok(Some(e)),
				Err(GltfImageError::EncodingToBCnDisabled) => Ok(None),
				Err(err) => Err(err),
			}
		} else {
			Ok(None)
		}
	}

	#[profiling::function]
	fn to_none_encode(&self, _: EncodeSettings) -> Result<Self, GltfImageError> {
		if self.metadata.runtime_compression() != RuntimeImageCompression::None {
			return Err(GltfImageError::EncodingFromBCn);
		}
		Ok(Self {
			metadata: Image2DMetadata {
				size: self.metadata.size,
				disk_compression: DiskImageCompression::None,
			},
			bytes: self.decode()?.into(),
		})
	}

	#[profiling::function]
	fn to_bc_encode(&self, settings: EncodeSettings) -> Result<Self, GltfImageError> {
		if self.metadata.runtime_compression() != RuntimeImageCompression::None {
			return Err(GltfImageError::EncodingFromBCn);
		}
		let src_size = self.metadata.size;
		let size = Size::new(src_size.width & !3, src_size.height & !3);
		let stride = src_size.width * self.metadata.image_type().channels();
		let bcn = match self.metadata.image_type() {
			ImageType::R_VALUES => {
				if settings.bc4_bc5 {
					profiling::scope!("bc4::compress_blocks");
					bc4::compress_blocks(&RSurface {
						height: size.height,
						width: size.width,
						stride,
						data: &self.decode()?,
					})
				} else {
					return Err(GltfImageError::EncodingToBCnDisabled);
				}
			}
			ImageType::RG_VALUES => {
				if settings.bc4_bc5 {
					profiling::scope!("bc5::compress_blocks");
					bc5::compress_blocks(&RgSurface {
						height: size.height,
						width: size.width,
						stride,
						data: &self.decode()?,
					})
				} else {
					return Err(GltfImageError::EncodingToBCnDisabled);
				}
			}
			ImageType::RGBA_LINEAR | ImageType::RGBA_COLOR => {
				if let Some(setting) = settings.bc7 {
					profiling::scope!("bc7::compress_blocks");
					let none = self.to_none_encode(settings)?;
					let has_alpha = scan_for_alpha(&none);
					bc7::compress_blocks(
						if has_alpha { &setting.alpha } else { &setting.opaque },
						&RgbaSurface {
							height: size.height,
							width: size.width,
							stride,
							data: &none.bytes,
						},
					)
				} else {
					return Err(GltfImageError::EncodingToBCnDisabled);
				}
			}
		};
		let bytes = zstd::bulk::compress(&bcn, settings.zstd_level)?.into();
		Ok(Image2DDisk {
			metadata: Image2DMetadata {
				size,
				disk_compression: DiskImageCompression::BCn_zstd,
			},
			bytes,
		})
	}
}

#[profiling::function]
fn scan_for_alpha<const IMAGE_TYPE: u32>(image: &Image2DDisk<IMAGE_TYPE>) -> bool {
	assert_eq!(image.metadata.disk_compression, DiskImageCompression::None);
	match image.metadata.image_type() {
		ImageType::R_VALUES => false,
		ImageType::RG_VALUES => false,
		ImageType::RGBA_LINEAR | ImageType::RGBA_COLOR => {
			assert_eq!(image.bytes.len() % 4, 0);
			for x in image.bytes.chunks_exact(4) {
				if x[3] != 255 {
					return true;
				}
			}
			false
		}
	}
}
