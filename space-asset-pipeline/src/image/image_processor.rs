use crate::gltf::{Gltf, GltfImageError, Scheme};
use crate::image::encode::{Encode, EncodeSettings};
use anyhow::Context;
use gltf::image::Source;
use gltf::Image;
use rayon::prelude::*;
use space_asset::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, ImageType, Size};
use std::ops::Deref;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use zune_image::codecs::png::zune_core::bytestream::ZCursor;
use zune_image::codecs::png::zune_core::options::DecoderOptions;
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;

const IMAGE_TYPES: usize = ImageType::IMAGE_TYPE_COUNT as usize;

pub struct ImageProcessor<'a> {
	gltf: &'a Gltf,
	requested_images: Box<[[AtomicBool; IMAGE_TYPES]]>,
}

impl<'a> Deref for ImageProcessor<'a> {
	type Target = Gltf;

	fn deref(&self) -> &Self::Target {
		&self.gltf
	}
}

impl<'a> ImageProcessor<'a> {
	pub fn new(gltf: &'a Gltf) -> Self {
		let mut requested_images = Vec::new();
		requested_images.resize_with(gltf.images().len(), <[AtomicBool; IMAGE_TYPES]>::default);

		Self {
			gltf,
			requested_images: requested_images.into_boxed_slice(),
		}
	}

	pub fn image<const IMAGE_TYPE: u32>(&self, image: Image) -> RequestedImage<IMAGE_TYPE> {
		let image_index = image.index();
		self.requested_images[image_index][IMAGE_TYPE as usize].store(true, Relaxed);
		RequestedImage { image_index }
	}

	pub fn process(self, settings: EncodeSettings) -> anyhow::Result<ImageAccessor> {
		profiling::scope!("ImageProcessor::process");
		let ImageProcessor { requested_images, gltf } = self;

		let vec = requested_images
			.into_vec()
			.into_par_iter()
			.map(|atomics| atomics.map(|b| b.into_inner()))
			.enumerate()
			.map(|(image_index, types): (usize, [bool; IMAGE_TYPES])| {
				if !types.iter().any(|b| *b) {
					return Ok((None, None, None, None));
				}

				let image = gltf.images().nth(image_index).unwrap();
				let scheme = match image.source() {
					Source::View { view, .. } => {
						let buffer = gltf.buffer(view.buffer()).ok_or(GltfImageError::MissingBuffer)?;
						Scheme::Slice(
							&buffer
								.get(view.offset()..view.length())
								.ok_or(GltfImageError::BufferViewOutOfBounds)?,
						)
					}
					Source::Uri { uri, .. } => Scheme::parse(uri).ok_or(GltfImageError::UnsupportedUri)?,
				};
				Self::process_individual_image(gltf, settings, &scheme, types)
					.with_context(|| format!("image scheme: {:?}", scheme))
			})
			.collect::<Result<Vec<_>, _>>()?;

		// unzip into 4 vecs
		let mut images_r_values = vec![None; vec.len()];
		let mut images_rg_values = vec![None; vec.len()];
		let mut images_rgba_linear = vec![None; vec.len()];
		let mut images_rgba_color = vec![None; vec.len()];
		for (i, (r_values, rg_values, rgba_linear, rgba_color)) in vec.into_iter().enumerate() {
			images_r_values[i] = r_values;
			images_rg_values[i] = rg_values;
			images_rgba_linear[i] = rgba_linear;
			images_rgba_color[i] = rgba_color;
		}

		Ok(ImageAccessor {
			images_r_values: images_r_values.into_boxed_slice(),
			images_rg_values: images_rg_values.into_boxed_slice(),
			images_rgba_linear: images_rgba_linear.into_boxed_slice(),
			images_rgba_color: images_rgba_color.into_boxed_slice(),
		})
	}

	fn process_individual_image(
		gltf: &'a Gltf,
		settings: EncodeSettings,
		scheme: &Scheme,
		types: [bool; IMAGE_TYPES],
	) -> anyhow::Result<(
		Option<Image2DDisk<{ ImageType::R_VALUES as u32 }>>,
		Option<Image2DDisk<{ ImageType::RG_VALUES as u32 }>>,
		Option<Image2DDisk<{ ImageType::RGBA_LINEAR as u32 }>>,
		Option<Image2DDisk<{ ImageType::RGBA_COLOR as u32 }>>,
	)> {
		profiling::scope!("process image");
		let bytes = {
			profiling::scope!("read into memory");
			Arc::<[u8]>::from(scheme.read(gltf.base())?)
		};
		let (format, _) = ImageFormat::guess_format(ZCursor::new(&bytes)).ok_or(GltfImageError::UnknownImageFormat)?;
		let image_metadata = {
			profiling::scope!("decode metadata");
			format
				.decoder_with_options(ZCursor::new(&bytes), DecoderOptions::new_fast())?
				.read_headers()
				.map_err(ImageErrors::from)?
				.expect("Image decoder reads metadata")
		};
		let size = Size::new(
			image_metadata.dimensions().0 as u32,
			image_metadata.dimensions().1 as u32,
		);

		fn into_optimal<const IMAGE_TYPE: u32>(
			size: Size,
			bytes: Arc<[u8]>,
			settings: EncodeSettings,
		) -> anyhow::Result<Image2DDisk<IMAGE_TYPE>> {
			profiling::scope!("into_optimal()", format!("{:?}", ImageType::try_from_const(IMAGE_TYPE)));
			Ok(Image2DDisk {
				metadata: Image2DMetadata {
					size,
					disk_compression: DiskImageCompression::Embedded,
				},
				bytes,
			}
			.into_optimal_encode(settings)
			.with_context(|| {
				format!(
					"into_optimal_encode::<{:?}>({:?}) failed",
					ImageType::try_from_const(IMAGE_TYPE),
					settings
				)
			})?)
		}
		let r_values = types[0]
			.then(|| into_optimal::<{ ImageType::R_VALUES as u32 }>(size, bytes.clone(), settings))
			.transpose()?;
		let rg_values = types[1]
			.then(|| into_optimal::<{ ImageType::RG_VALUES as u32 }>(size, bytes.clone(), settings))
			.transpose()?;
		let rgba_linear = types[2]
			.then(|| into_optimal::<{ ImageType::RGBA_LINEAR as u32 }>(size, bytes.clone(), settings))
			.transpose()?;
		let rgba_color = types[3]
			.then(|| into_optimal::<{ ImageType::RGBA_COLOR as u32 }>(size, bytes.clone(), settings))
			.transpose()?;
		Ok((r_values, rg_values, rgba_linear, rgba_color))
	}
}

pub struct ImageAccessor {
	images_r_values: Box<[Option<Image2DDisk<{ ImageType::R_VALUES as u32 }>>]>,
	images_rg_values: Box<[Option<Image2DDisk<{ ImageType::RG_VALUES as u32 }>>]>,
	images_rgba_linear: Box<[Option<Image2DDisk<{ ImageType::RGBA_LINEAR as u32 }>>]>,
	images_rgba_color: Box<[Option<Image2DDisk<{ ImageType::RGBA_COLOR as u32 }>>]>,
}

pub struct RequestedImage<const IMAGE_TYPE: u32> {
	image_index: usize,
}

impl<const IMAGE_TYPE: u32> RequestedImage<IMAGE_TYPE> {
	pub const fn image_type(&self) -> ImageType {
		ImageType::try_from_const(IMAGE_TYPE)
	}
}

impl RequestedImage<{ ImageType::R_VALUES as u32 }> {
	pub fn get(&self, access: &ImageAccessor) -> Image2DDisk<{ ImageType::R_VALUES as u32 }> {
		access.images_r_values[self.image_index].clone().unwrap()
	}
}

impl RequestedImage<{ ImageType::RG_VALUES as u32 }> {
	pub fn get(&self, access: &ImageAccessor) -> Image2DDisk<{ ImageType::RG_VALUES as u32 }> {
		access.images_rg_values[self.image_index].clone().unwrap()
	}
}

impl RequestedImage<{ ImageType::RGBA_LINEAR as u32 }> {
	pub fn get(&self, access: &ImageAccessor) -> Image2DDisk<{ ImageType::RGBA_LINEAR as u32 }> {
		access.images_rgba_linear[self.image_index].clone().unwrap()
	}
}

impl RequestedImage<{ ImageType::RGBA_COLOR as u32 }> {
	pub fn get(&self, access: &ImageAccessor) -> Image2DDisk<{ ImageType::RGBA_COLOR as u32 }> {
		access.images_rgba_color[self.image_index].clone().unwrap()
	}
}
