use crate::gltf::uri::Scheme;
use glam::{Affine3A, Quat, Vec3};
use gltf::buffer::Data;
use gltf::image::Source;
use gltf::{Buffer, Document, Image, Node, Scene};
use smallvec::SmallVec;
use space_asset::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, Size};
use std::fmt::{Display, Formatter};
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use zune_image::codecs::png::zune_core::bytestream::ZCursor;
use zune_image::codecs::png::zune_core::options::DecoderOptions;
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;

pub struct Gltf {
	pub document: Document,
	pub base: PathBuf,
	pub buffers: SmallVec<[Data; 1]>,
}

impl Gltf {
	#[profiling::function]
	pub fn open(path: &Path) -> Result<Self, gltf::Error> {
		let base = path
			.parent()
			.map(Path::to_path_buf)
			.unwrap_or_else(|| PathBuf::from("./"));
		let gltf::Gltf { document, mut blob } = gltf::Gltf::open(&path)?;
		let buffers = document
			.buffers()
			.map(|buffer| Data::from_source_and_blob(buffer.source(), Some(base.as_path()), &mut blob))
			.collect::<Result<_, _>>()?;
		Ok(Self {
			document,
			base,
			buffers,
		})
	}

	pub fn base(&self) -> &Path {
		self.base.as_path()
	}

	pub fn buffer(&self, buffer: Buffer) -> Option<&[u8]> {
		self.buffers.get(buffer.index()).map(|b| &b.0[..])
	}
}

impl Deref for Gltf {
	type Target = Document;

	fn deref(&self) -> &Self::Target {
		&self.document
	}
}

#[derive(Debug)]
pub enum GltfImageError {
	MissingBuffer,
	BufferViewOutOfBounds,
	UnsupportedUri,
	UnknownImageFormat,
	EncodingFromBCn,
	EncodingToBCnDisabled,
	ImageErrors(ImageErrors),
	IoError(io::Error),
}

impl Display for GltfImageError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			GltfImageError::MissingBuffer => f.write_str("Invalid buffer index"),
			GltfImageError::BufferViewOutOfBounds => f.write_str("Buffer view is out of bounds"),
			GltfImageError::UnsupportedUri => f.write_str("Image URI is unsupported or invalid"),
			GltfImageError::UnknownImageFormat => f.write_str("Image format is unknown"),
			GltfImageError::EncodingFromBCn => f.write_str("Cannot encode BCn image into another format"),
			GltfImageError::EncodingToBCnDisabled => {
				f.write_str("Encoding into suitable BCn format disabled by settings")
			}
			GltfImageError::ImageErrors(err) => Display::fmt(err, f),
			GltfImageError::IoError(err) => Display::fmt(err, f),
		}
	}
}

impl std::error::Error for GltfImageError {}

impl From<ImageErrors> for GltfImageError {
	fn from(value: ImageErrors) -> Self {
		Self::ImageErrors(value)
	}
}

impl From<io::Error> for GltfImageError {
	fn from(value: io::Error) -> Self {
		Self::IoError(value)
	}
}

impl Gltf {
	#[profiling::function]
	pub fn image<const DATA_TYPE: u32>(&self, image: Image) -> Result<Image2DDisk<DATA_TYPE>, GltfImageError> {
		let scheme = match image.source() {
			Source::View { view, .. } => {
				let buffer = self.buffer(view.buffer()).ok_or(GltfImageError::MissingBuffer)?;
				Scheme::Slice(
					&buffer
						.get(view.offset()..view.length())
						.ok_or(GltfImageError::BufferViewOutOfBounds)?,
				)
			}
			Source::Uri { uri, .. } => Scheme::parse(uri).ok_or(GltfImageError::UnsupportedUri)?,
		};

		let src = {
			profiling::scope!("read into memory");
			scheme.read(self.base())?
		};
		let (format, _) = ImageFormat::guess_format(ZCursor::new(&src)).ok_or(GltfImageError::UnknownImageFormat)?;
		let metadata = {
			profiling::scope!("decode metadata");
			format
				.decoder_with_options(ZCursor::new(&src), DecoderOptions::new_fast())?
				.read_headers()
				.map_err(ImageErrors::from)?
				.expect("Image decoder reads metadata")
		};
		let size = Size::new(metadata.dimensions().0 as u32, metadata.dimensions().1 as u32);

		Ok(Image2DDisk {
			metadata: Image2DMetadata {
				size,
				disk_compression: DiskImageCompression::Embedded,
			},
			bytes: src.into(),
		})
	}
}

impl Gltf {
	#[profiling::function]
	pub fn absolute_node_transformations(&self, scene: &Scene, base: Affine3A) -> Vec<Affine3A> {
		fn walk(out: &mut Vec<Affine3A>, node: Node, parent: Affine3A) {
			let (translation, rotation, scale) = node.transform().decomposed();
			let node_absolute = parent
				* Affine3A::from_scale_rotation_translation(
					Vec3::from(scale),
					Quat::from_array(rotation),
					Vec3::from(translation),
				);
			out[node.index()] = node_absolute;
			for node in node.children() {
				walk(out, node, node_absolute);
			}
		}

		let mut out = vec![Affine3A::IDENTITY; self.nodes().len()];
		for node in scene.nodes() {
			walk(&mut out, node, base);
		}
		out
	}
}
