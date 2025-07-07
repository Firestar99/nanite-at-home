use space_asset_disk::image::{DecodeToRuntimeImage, ImageType, RuntimeImageMetadata, UncompressedImage};

/// Generates `mip_levels` many mip images, removes and regenerates previous mip levels if present.
pub fn generate_mips(image: &mut UncompressedImage, mip_levels: Option<u32>) {
	profiling::function_scope!();
	let mip_levels = mip_levels.unwrap_or(RuntimeImageMetadata::complete_mip_chain_levels(image.meta.extent));
	image.meta.mip_levels = mip_levels;
	let meta = image.meta.decoded_metadata();

	let data = image.data.to_mut();
	data.drain(meta.mip_start(1)..);
	data.reserve_exact(meta.total_size - data.len());

	match meta.image_type {
		ImageType::RValue => generate_mips_inner::<1>(data, meta),
		ImageType::RgValue => generate_mips_inner::<2>(data, meta),
		ImageType::RgbaLinear | ImageType::RgbaColor => generate_mips_inner::<4>(data, meta),
	}
}

// TODO doesn't support 3d textures
#[allow(clippy::needless_range_loop)]
fn generate_mips_inner<const CHANNELS: usize>(data: &mut Vec<u8>, meta: RuntimeImageMetadata) {
	profiling::function_scope!();

	for mip in 1..meta.mip_levels {
		profiling::scope!("mip level", &format!("{}", mip));
		let read_extent = meta.mip_extent(mip - 1);
		let write_extent = meta.mip_extent(mip);

		for y in 0..write_extent.y {
			for x in 0..write_extent.x {
				let mut sum = [0.; CHANNELS];
				let mut div = 0.;
				for read_x in 0..u32::min(2, read_extent.x) {
					for read_y in 0..u32::min(2, read_extent.y) {
						let read_slice = &data[meta.mip_range(mip - 1)];
						let read_offset = ((y * 2 + read_y) * read_extent.x + (x * 2 + read_x)) as usize * CHANNELS;
						for ch in 0..CHANNELS {
							sum[ch] += read_slice[read_offset + ch] as f32;
						}
						div += 1.;
					}
				}
				for ch in 0..CHANNELS {
					data.push(f32::round(sum[ch] / div) as u8);
				}
			}
		}
		assert_eq!(data.len(), (0..=mip).map(|i| meta.mip_size(i)).sum::<usize>());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::UVec3;
	use space_asset_disk::image::UncompressedImageMetadata;
	use std::borrow::Cow;

	#[test]
	fn test_generate_mips() {
		let data = [
			[1, 2, 3, 4, 5, 6, 7, 8],
			[1, 2, 3, 4, 5, 6, 7, 8],
			[1, 2, 3, 4, 5, 6, 7, 8],
			[1, 2, 3, 4, 5, 6, 7, 8],
		];
		let mut image = UncompressedImage {
			meta: UncompressedImageMetadata {
				image_type: ImageType::RValue,
				extent: UVec3::new(8, 4, 1),
				mip_levels: 1,
			},
			data: Cow::Owned(data.iter().flatten().copied().collect::<Vec<u8>>()),
		};
		generate_mips(&mut image, None);

		let meta = image.meta.decoded_metadata();
		assert_eq!(meta.mip_levels, 4);

		let mip0 = &image.data[meta.mip_range(0)];
		assert_eq!(mip0, data.iter().flatten().copied().collect::<Vec<u8>>());

		let mip1 = &image.data[meta.mip_range(1)];
		let expected_mip1 = [[2, 4, 6, 8], [2, 4, 6, 8]];
		assert_eq!(mip1, expected_mip1.iter().flatten().copied().collect::<Vec<u8>>());

		let mip2 = &image.data[meta.mip_range(2)];
		assert_eq!(mip2, &[3, 7]);

		let mip3 = &image.data[meta.mip_range(3)];
		assert_eq!(mip3, &[5]);
	}
}
