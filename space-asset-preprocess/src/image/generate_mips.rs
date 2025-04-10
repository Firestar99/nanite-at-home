use glam::{UVec2, UVec4};
use space_asset_disk::image::Size;

#[derive(Copy, Clone)]
pub struct MipLevels {
	pub size: Size,
	pub mip_levels: u32,
	pub pixels: u32,
	pub bytes: usize,
}

impl MipLevels {
	pub fn new(size: Size, mip_levels: u32) -> MipLevels {
		let pixels = {
			let mut pixels = 0;
			let mut size = UVec2::from(size);
			for _ in 0..mip_levels {
				pixels += size.x * size.y;
				size /= 2;
			}
			pixels
		};

		MipLevels {
			size,
			mip_levels,
			pixels,
			bytes: pixels as usize * 4, // rgba
		}
	}
}

pub fn optimal_mip_levels(size: Size, max_mip_levels: Option<u32>) -> MipLevels {
	let mut mip_levels = 0;
	let mut extent = UVec2::from(size);
	loop {
		if extent % 2 != UVec2::ZERO || max_mip_levels.map_or(false, |max| mip_levels >= max) {
			break;
		}
		extent /= 2;
		mip_levels += 1;
	}

	if let Some(max_mip_levels) = max_mip_levels {
		assert!(mip_levels <= max_mip_levels);
	}
	MipLevels::new(size, mip_levels)
}

#[derive(Clone)]
pub struct MipImage {
	pub size: Size,
	pub bytes: Vec<u8>,
}

/// this could TOCTOU if MipLevels is changed
pub fn generate_mips(mip_levels: MipLevels, bytes: &Vec<u8>) -> Vec<MipImage> {
	profiling::function_scope!();
	let mut out = Vec::with_capacity(mip_levels.mip_levels as usize);

	let mut input = bytes;
	let mut size = UVec2::from(mip_levels.size) / 2;
	for _i in 0..mip_levels.mip_levels {
		profiling::scope!("mip level", &format!("{}", _i));

		let output_cap = (size.x * size.y) as usize * 4;
		let mut output = Vec::with_capacity(output_cap);
		for x in 0..size.x {
			for y in 0..size.y {
				let mut sum = UVec4::ZERO;
				for read_x in 0..2 {
					for read_y in 0..2 {
						let read_offset = ((y + read_y) * size.x + (x + read_x)) as usize * 4;
						sum += UVec4::from_array([0, 1, 2, 3].map(|i| input[read_offset + i] as u32));
					}
				}
				[0, 1, 2, 3].map(|i| {
					// take the average and round properly
					output.push((sum[i] / 4 + u32::from(sum[i] % 4 < 1)) as u8)
				});
			}
		}
		assert_eq!(output.len(), output_cap);
		out.push(MipImage {
			size: Size::from(size),
			bytes: output,
		});

		size /= 2;
		input = &out.last().unwrap().bytes;
	}
	out
}
