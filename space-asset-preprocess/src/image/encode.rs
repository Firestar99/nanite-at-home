use crate::gltf::GltfImageError;
use crate::image::generate_mips::{generate_mips, optimal_mip_levels};
use intel_tex_2::{bc4, bc5, bc7, RSurface, RgSurface, RgbaSurface};
use rayon::prelude::*;
use space_asset_disk::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, ImageType, RuntimeImageCompression};

#[derive(Copy, Clone, Debug)]
pub struct Bc7Settings {
	opaque: bc7::EncodeSettings,
	alpha: bc7::EncodeSettings,
}

#[derive(Copy, Clone, Debug)]
pub struct EncodeSettings {
	bc4_bc5: bool,
	bc7: Option<Bc7Settings>,
	max_mip_levels: Option<u32>,
	zstd_level: i32,
}

impl EncodeSettings {
	pub fn embedded() -> Self {
		Self {
			bc4_bc5: false,
			bc7: None,
			max_mip_levels: Some(0),
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
			max_mip_levels: None,
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
			max_mip_levels: None,
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
			max_mip_levels: None,
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
			max_mip_levels: None,
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
			max_mip_levels: None,
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
	fn to_optimal_encode(&self, settings: EncodeSettings) -> Result<Option<Self>, GltfImageError> {
		profiling::function_scope!();
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

	fn to_none_encode(&self, _: EncodeSettings) -> Result<Self, GltfImageError> {
		profiling::function_scope!();
		if self.metadata.runtime_compression() != RuntimeImageCompression::None {
			return Err(GltfImageError::EncodingFromBCn);
		}
		let decoded = self.decode()?;
		Ok(Self {
			metadata: Image2DMetadata {
				size: self.metadata.size,
				mip_levels: self.metadata.mip_levels,
				disk_compression: DiskImageCompression::None,
				decompressed_size: decoded.len(),
			},
			bytes: decoded.into(),
		})
	}

	fn to_bc_encode(&self, settings: EncodeSettings) -> Result<Self, GltfImageError> {
		profiling::function_scope!();
		if self.metadata.runtime_compression() != RuntimeImageCompression::None {
			return Err(GltfImageError::EncodingFromBCn);
		}

		let decoded = self.decode()?;
		let has_alpha = rgba_image_has_any_alpha(&decoded);
		let mip_levels = optimal_mip_levels(self.metadata.size, settings.max_mip_levels);
		let mip_images = generate_mips(mip_levels, &decoded);

		let bcn = mip_images
			.par_iter()
			.map(|mip_image| match self.metadata.image_type() {
				ImageType::R_VALUES => {
					profiling::scope!("bc4::compress_blocks");
					bc4::compress_blocks(&RSurface {
						height: mip_image.size.height,
						width: mip_image.size.width,
						stride: mip_image.size.width,
						data: &mip_image.bytes,
					})
				}
				ImageType::RG_VALUES => {
					profiling::scope!("bc5::compress_blocks");
					bc5::compress_blocks(&RgSurface {
						height: mip_image.size.height,
						width: mip_image.size.width,
						stride: mip_image.size.width * 2,
						data: &mip_image.bytes,
					})
				}
				ImageType::RGBA_LINEAR | ImageType::RGBA_COLOR => {
					if let Some(setting) = settings.bc7 {
						profiling::scope!("bc7::compress_blocks");
						bc7::compress_blocks(
							if has_alpha { &setting.alpha } else { &setting.opaque },
							&RgbaSurface {
								height: mip_image.size.height,
								width: mip_image.size.width,
								stride: mip_image.size.width * 4,
								data: &mip_image.bytes,
							},
						)
					} else {
						panic!("{:?}", GltfImageError::EncodingToBCnDisabled);
					}
				}
			})
			.flatten()
			.collect::<Vec<_>>();

		let decompressed_size = bcn.len();
		let bytes = zstd::bulk::compress(&bcn, settings.zstd_level)?.into();
		Ok(Image2DDisk {
			metadata: Image2DMetadata {
				size: self.metadata.size,
				mip_levels: mip_levels.mip_levels,
				disk_compression: DiskImageCompression::BCn_zstd,
				decompressed_size,
			},
			bytes,
		})
	}
}

fn rgba_image_has_any_alpha(image: &[u8]) -> bool {
	profiling::function_scope!();
	assert_eq!(image.len() % 4, 0);
	for x in image.chunks_exact(4) {
		if x[3] != 255 {
			return true;
		}
	}
	false
}
