use crate::image::{BCnImage, BCnImageMetadata, DecodeToRuntimeImage, ImageType, UncompressedImage, ZstdBCnImage};
use intel_tex_2::{bc4, bc5, bc7, RSurface, RgSurface, RgbaSurface};
use std::borrow::Cow;

impl<const IMAGE_TYPE: u32> UncompressedImage<'_, IMAGE_TYPE> {
	pub fn compress_to_bcn(&self, settings: Option<Bc7Settings>) -> BCnImage<'static, IMAGE_TYPE> {
		profiling::function_scope!();
		let bcn_meta = BCnImageMetadata {
			extent: self.meta.extent,
			mip_layers: self.meta.mip_layers,
		};
		let meta_in = self.meta.decoded_metadata();
		let meta_out = bcn_meta.decoded_metadata();
		let mut data = vec![0; meta_out.total_size];

		for mip in 0..meta_out.mip_layers {
			let mip_extent = meta_in.mip_extent(mip);
			let pixels_in = &self.data[meta_in.mip_start(mip)..meta_in.mip_size(mip)];
			let blocks_out = &mut data[meta_out.mip_start(mip)..meta_out.mip_size(mip)];
			match ImageType::try_from_const(IMAGE_TYPE) {
				ImageType::R_VALUE => {
					profiling::scope!("bc4::compress_blocks");
					bc4::compress_blocks_into(
						&RSurface {
							height: mip_extent.y,
							width: mip_extent.x,
							stride: mip_extent.x * 1,
							data: pixels_in,
						},
						blocks_out,
					)
				}
				ImageType::RG_VALUE => {
					profiling::scope!("bc5::compress_blocks");
					bc5::compress_blocks_into(
						&RgSurface {
							height: mip_extent.y,
							width: mip_extent.x,
							stride: mip_extent.x * 2,
							data: pixels_in,
						},
						blocks_out,
					)
				}
				ImageType::RGBA_LINEAR | ImageType::RGBA_COLOR => {
					let setting = settings.expect("Compressing to bc7 requires Bc7Settings");
					profiling::scope!("bc7::compress_blocks");
					let has_alpha = scan_for_alpha::<IMAGE_TYPE>(&pixels_in);
					bc7::compress_blocks_into(
						if has_alpha { &setting.alpha } else { &setting.opaque },
						&RgbaSurface {
							height: mip_extent.y,
							width: mip_extent.x,
							stride: mip_extent.x * 4,
							data: pixels_in,
						},
						blocks_out,
					)
				}
			};
		}

		BCnImage {
			meta: bcn_meta,
			data: Cow::Owned(data),
		}
	}

	pub fn compress_to_zstd_bcn(&self, settings: EncodeSettings) -> ZstdBCnImage<'static, IMAGE_TYPE> {
		self.compress_to_bcn(settings.bc7).compress_to_zstd(settings.zstd_level)
	}
}

fn scan_for_alpha<const IMAGE_TYPE: u32>(pixels: &[u8]) -> bool {
	profiling::function_scope!();
	match ImageType::try_from_const(IMAGE_TYPE) {
		ImageType::R_VALUE => false,
		ImageType::RG_VALUE => false,
		ImageType::RGBA_LINEAR | ImageType::RGBA_COLOR => {
			assert_eq!(pixels.len() % 4, 0);
			for x in pixels.chunks_exact(4) {
				if x[3] != 255 {
					return true;
				}
			}
			false
		}
	}
}

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
