use crate::image::{BCnImage, BCnImageMetadata, DecodeToRuntimeImage, ImageType, UncompressedImage, ZstdBCnImage};
use intel_tex_2::{bc4, bc5, bc7, RSurface, RgSurface, RgbaSurface};
use std::borrow::Cow;

impl UncompressedImage<'_> {
	pub fn compress_to_bcn(&self, settings: Option<Bc7Settings>) -> BCnImage<'static> {
		profiling::function_scope!();
		let bcn_meta = BCnImageMetadata {
			image_type: self.meta.image_type,
			extent: self.meta.extent,
			mip_levels: self.meta.mip_levels,
		};
		let meta_in = self.meta.decoded_metadata();
		let meta_out = bcn_meta.decoded_metadata();
		let mut data = vec![0; meta_out.total_size];

		for mip in 0..meta_out.mip_levels {
			let mip_extent = meta_in.mip_extent(mip);
			let pixels_in = &self.data[meta_in.mip_start(mip)..meta_in.mip_size(mip)];
			let blocks_out = &mut data[meta_out.mip_start(mip)..meta_out.mip_size(mip)];
			match self.meta.image_type {
				ImageType::RValue => {
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
				ImageType::RgValue => {
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
				ImageType::RgbaLinear | ImageType::RgbaColor => {
					let setting = settings.expect("Compressing to bc7 requires Bc7Settings");
					profiling::scope!("bc7::compress_blocks");
					let has_alpha = scan_for_alpha(self.meta.image_type, &pixels_in);
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

	pub fn compress_to_zstd_bcn(&self, settings: EncodeSettings) -> ZstdBCnImage<'static> {
		self.compress_to_bcn(settings.bc7).compress_to_zstd(settings.zstd_level)
	}
}

fn scan_for_alpha(image_type: ImageType, pixels: &[u8]) -> bool {
	profiling::function_scope!();
	match image_type {
		ImageType::RValue => false,
		ImageType::RgValue => false,
		ImageType::RgbaLinear | ImageType::RgbaColor => {
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
