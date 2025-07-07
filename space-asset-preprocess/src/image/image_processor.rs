use crate::gltf::{Gltf, GltfImageError, Scheme};
use crate::image::generate_mips::generate_mips;
use anyhow::Context;
use gltf::image::Source;
use parking_lot::Mutex;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use space_asset_disk::image::{DynImage, EmbeddedImage, EncodeSettings, ImageDiskTrait, ImageStorage, ImageType};
use std::ops::Deref;

pub struct ImageProcessor<'a> {
	gltf: &'a Gltf,
	inner: Mutex<Inner>,
}

struct Inner {
	next_disk_id: usize,
	gltf_image_to_disk_id: FxHashMap<(usize, ImageType), (usize, String)>,
}

impl Deref for ImageProcessor<'_> {
	type Target = Gltf;

	fn deref(&self) -> &Self::Target {
		self.gltf
	}
}

impl<'a> ImageProcessor<'a> {
	pub fn new(gltf: &'a Gltf) -> Self {
		Self {
			gltf,
			inner: Mutex::new(Inner {
				next_disk_id: 0,
				gltf_image_to_disk_id: FxHashMap::default(),
			}),
		}
	}

	pub fn image<I: ImageDiskTrait>(&self, image: gltf::Image, name: String) -> I {
		let key = (image.index(), I::IMAGE_TYPE);
		let mut inner = self.inner.lock();
		let Inner {
			next_disk_id,
			gltf_image_to_disk_id,
		} = &mut *inner;
		let image_id = gltf_image_to_disk_id
			.entry(key)
			.or_insert_with(|| {
				let old = *next_disk_id;
				*next_disk_id += 1;
				(old, name)
			})
			.0;
		I::new(image_id)
	}

	pub fn process(self, settings: EncodeSettings) -> anyhow::Result<ImageStorage> {
		profiling::scope!("ImageProcessor::process");
		let ImageProcessor { gltf, inner } = self;
		let Inner {
			gltf_image_to_disk_id,
			next_disk_id,
		} = inner.into_inner();

		let dyn_images = gltf_image_to_disk_id
			.into_par_iter()
			.panic_fuse()
			.map(|((image_index, image_type), disk_id)| {
				let image = gltf.images().nth(image_index).unwrap();
				let scheme = match image.source() {
					Source::View { view, .. } => {
						let buffer = gltf.buffer(view.buffer()).ok_or(GltfImageError::MissingBuffer)?;
						Scheme::Slice(
							buffer
								.get(view.offset()..(view.offset() + view.length()))
								.ok_or(GltfImageError::BufferViewOutOfBounds)?,
						)
					}
					Source::Uri { uri, .. } => Scheme::parse(uri).ok_or(GltfImageError::UnsupportedUri)?,
				};
				let image = Self::process_individual_image(gltf, &scheme, image_type, settings)
					.with_context(|| format!("gltf image {image_index} scheme {scheme:?}"))?;
				Ok((disk_id, image))
			})
			.collect::<anyhow::Result<Vec<_>>>()?;

		let mut disk_images = vec![None; next_disk_id];
		for ((disk_id, name), image) in dyn_images {
			disk_images[disk_id] = Some((image, name));
		}

		Ok(ImageStorage {
			images: disk_images
				.into_iter()
				.collect::<Option<Vec<_>>>()
				.expect("sequential image ids without holes"),
		})
	}

	#[allow(clippy::type_complexity)]
	fn process_individual_image(
		gltf: &'a Gltf,
		scheme: &Scheme,
		image_type: ImageType,
		settings: EncodeSettings,
	) -> anyhow::Result<DynImage<'static>> {
		profiling::function_scope!(&format!("process image {:?}", scheme));
		let bytes = {
			profiling::scope!("read into memory");
			scheme.read(gltf.base())?
		};
		let embedded_image = EmbeddedImage::new(image_type, bytes)?;
		let mut uncompressed_image = embedded_image.decode_to_uncompressed();
		drop(embedded_image);
		generate_mips(&mut uncompressed_image, None);
		let zstd_bcn_image = uncompressed_image.compress_to_zstd_bcn(settings);
		drop(uncompressed_image);
		Ok(zstd_bcn_image.into_dyn_image())
	}
}
