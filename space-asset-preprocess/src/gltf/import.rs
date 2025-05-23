use crate::gltf::Scheme;
use glam::{Affine3A, Quat, Vec3};
use gltf::buffer::Source;
use gltf::{Buffer, Document, Node, Scene};
use smallvec::SmallVec;
use std::fmt::{Display, Formatter};
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};

pub struct Gltf {
	pub document: Document,
	pub base: PathBuf,
	pub buffers: SmallVec<[Vec<u8>; 1]>,
}

impl Gltf {
	pub fn open(path: &Path) -> anyhow::Result<Self> {
		profiling::function_scope!();
		let base = path
			.parent()
			.map(Path::to_path_buf)
			.unwrap_or_else(|| PathBuf::from("./"));
		let gltf::Gltf { document, mut blob } = gltf::Gltf::open(path)?;
		let buffers = document
			.buffers()
			.map(|buffer| Self::load_buffer(buffer, base.as_path(), &mut blob))
			.collect::<Result<_, _>>()?;
		Ok(Self {
			document,
			base,
			buffers,
		})
	}

	fn load_buffer(buffer: Buffer, base_path: &Path, blob: &mut Option<Vec<u8>>) -> Result<Vec<u8>, GltfImageError> {
		Ok(match buffer.source() {
			Source::Bin => blob.take().ok_or(GltfImageError::MissingBuffer)?,
			Source::Uri(uri) => Scheme::parse(uri)
				.ok_or(GltfImageError::UnsupportedUri)?
				.read(base_path)?
				.into_owned(),
		})
	}

	pub fn base(&self) -> &Path {
		self.base.as_path()
	}

	pub fn buffer(&self, buffer: Buffer) -> Option<&[u8]> {
		self.buffers.get(buffer.index()).map(|b| &**b)
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
			GltfImageError::IoError(err) => Display::fmt(err, f),
		}
	}
}

impl std::error::Error for GltfImageError {}

impl From<io::Error> for GltfImageError {
	fn from(value: io::Error) -> Self {
		Self::IoError(value)
	}
}

impl Gltf {
	pub fn absolute_node_transformations(&self, scene: &Scene, base: Affine3A) -> Vec<Affine3A> {
		profiling::function_scope!();
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
