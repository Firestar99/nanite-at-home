// #[repr(u8)]
// #[derive(Copy, Clone, Debug)]
// pub enum SourceImageFormat {
// 	Png,
// 	Jpg,
// }
//
// impl SourceImageFormat {
// 	// FIXME remove?
// 	// pub fn from_mime_type(mine_type: &str) -> Option<Self> {
// 	// 	match mine_type {
// 	// 		"image/png" => Some(Self::Png),
// 	// 		"image/jpeg" => Some(Self::Jpg),
// 	// 		_ => None,
// 	// 	}
// 	// }
// 	//
// 	// pub fn from_file_ending(file_ending: &str) -> Option<Self> {
// 	// 	match &*file_ending.to_ascii_lowercase() {
// 	// 		"png" => Some(Self::Png),
// 	// 		"jpg" => Some(Self::Jpg),
// 	// 		"jpeg" => Some(Self::Jpg),
// 	// 		_ => None,
// 	// 	}
// 	// }
//
// 	pub fn from_file_magic(file_magic: &[u8; 8]) -> Option<Self> {
// 		if file_magic.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
// 			Some(Self::Png)
// 		} else if file_magic.starts_with(&[0xff, 0xd8, 0xff]) {
// 			Some(Self::Jpg)
// 		} else {
// 			None
// 		}
// 	}
//
// 	pub fn to_disk_image_compression(&self) -> DiskImageCompression {
// 		match self {
// 			SourceImageFormat::Png => DiskImageCompression::Png,
// 			SourceImageFormat::Jpg => DiskImageCompression::Jpg,
// 		}
// 	}
//
// 	pub fn decode_metadata<R: Read>(&self, read: R) -> Image2DMetadata {
// 		match self {
// 			SourceImageFormat::Png => {
// 				let reader = png::Decoder::new(read).read_info()?;
// 				let info = reader.info();
// 				Image2DMetadata {
// 					size: Size::new(info.width, info.height),
// 					data_type: info.color_type,
// 				}
// 			}
// 			SourceImageFormat::Jpg => {}
// 		}
// 	}
// }

// pub trait ImageExt {
// 	fn encode_rgba(image: DynamicImage) -> Self;
// 	fn encode_normal_map(image: DynamicImage) -> Self;
// }
//
// #[inline]
// fn should_compress(encoding: ImageEncoding, size: Size) -> bool {
// 	let block_size = encoding.block_size();
// 	size.width >= block_size.width && size.height >= block_size.height
// 	// false
// }
//
// impl ImageExt for Image2DDisk {
// 	#[profiling::function]
// 	fn encode_rgba(image: DynamicImage) -> Self {
// 		let image = {
// 			profiling::scope!("convert to rgba8 image");
// 			image.into_rgba8()
// 		};
//
// 		let size = Size::new(image.width(), image.height());
// 		if should_compress(ImageEncoding::BC7_SRGB, size) {
// 			profiling::scope!("bc7::compress_blocks");
// 			let bytes = bc7::compress_blocks(
// 				&bc7::opaque_ultra_fast_settings(),
// 				&RgbaSurface {
// 					height: size.height,
// 					width: size.width,
// 					stride: size.width * 4,
// 					data: image.as_bytes(),
// 				},
// 			);
// 			Self {
// 				metadata: Image2DMetadata {
// 					encoding: ImageEncoding::BC7_SRGB,
// 					size,
// 				},
// 				bytes: bytes.into_boxed_slice(),
// 			}
// 		} else {
// 			Self {
// 				metadata: Image2DMetadata {
// 					encoding: ImageEncoding::RGBA_SRGB,
// 					size,
// 				},
// 				bytes: image.into_raw().into_boxed_slice(),
// 			}
// 		}
// 	}
//
// 	#[profiling::function]
// 	fn encode_normal_map(image: DynamicImage) -> Self {
// 		let size = Size::new(image.width(), image.height());
// 		let rg_image = {
// 			profiling::scope!("convert to rg8 image");
// 			let mut rg_image = vec![0u8; size.width as usize * size.height as usize * 2].into_boxed_slice();
// 			for (x, y, pixel) in image.pixels() {
// 				let offset = y as usize * size.width as usize + x as usize;
// 				rg_image[offset] = pixel.0[0];
// 				rg_image[offset + 1] = pixel.0[1];
// 			}
// 			rg_image
// 		};
//
// 		if should_compress(ImageEncoding::BC5_UNORM, size) {
// 			profiling::scope!("bc5::compress_blocks");
// 			let bytes = bc5::compress_blocks(&RgSurface {
// 				height: size.height,
// 				width: size.width,
// 				stride: size.width * 2,
// 				data: &rg_image,
// 			});
// 			Self {
// 				metadata: Image2DMetadata {
// 					encoding: ImageEncoding::BC5_UNORM,
// 					size,
// 				},
// 				bytes: bytes.into_boxed_slice(),
// 			}
// 		} else {
// 			Self {
// 				metadata: Image2DMetadata {
// 					encoding: ImageEncoding::RG_UNORM,
// 					size,
// 				},
// 				bytes: rg_image,
// 			}
// 		}
// 	}
// }
