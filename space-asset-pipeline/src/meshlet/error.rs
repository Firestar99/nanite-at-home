use crate::meshlet::process::GltfImageError;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub enum Error {
	Gltf(gltf::Error),
	Io(std::io::Error),
	Image(GltfImageError),
	Meshlet(MeshletError),
}

impl Display for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::Gltf(err) => Display::fmt(err, f),
			Error::Io(err) => Display::fmt(err, f),
			Error::Image(err) => Display::fmt(err, f),
			Error::Meshlet(err) => Display::fmt(err, f),
		}
	}
}

impl std::error::Error for Error {}

impl From<gltf::Error> for Error {
	fn from(value: gltf::Error) -> Self {
		match value {
			gltf::Error::Io(err) => Self::Io(err),
			err => Self::Gltf(err),
		}
	}
}

impl From<std::io::Error> for Error {
	fn from(value: std::io::Error) -> Self {
		Self::Io(value)
	}
}

impl From<GltfImageError> for Error {
	fn from(value: GltfImageError) -> Self {
		Self::Image(value)
	}
}

impl From<MeshletError> for Error {
	fn from(value: MeshletError) -> Self {
		Self::Meshlet(value)
	}
}

#[derive(Debug)]
pub enum MeshletError {
	PrimitiveMustBeTriangleList,
	NoVertexPositions,
	NoTextureCoords,
	NoNormals,
	MultipleTextureCoords,
	MissingTextures,
	NoDefaultScene,
}

impl Display for MeshletError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			MeshletError::PrimitiveMustBeTriangleList => f.write_str("All primitives must be triangle lists"),
			MeshletError::NoVertexPositions => f.write_str("A mesh primitive exists with no vertex positions"),
			MeshletError::NoTextureCoords => f.write_str("A mesh primitive exists with no texture coordinates"),
			MeshletError::NoNormals => f.write_str("A mesh primitive exists with no normals"),
			MeshletError::MultipleTextureCoords => {
				f.write_str("Mesh uses multiple texture coordinates for their materials")
			}
			MeshletError::MissingTextures => f.write_str("Some textures were missing"),
			MeshletError::NoDefaultScene => f.write_str("No default scene exists"),
		}
	}
}

impl std::error::Error for MeshletError {}

pub type Result<T> = core::result::Result<T, Error>;
