use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
	Gltf(gltf::Error),
	Io(std::io::Error),
	Meshlet(MeshletError),
}

impl Display for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::Gltf(err) => err.fmt(f),
			Error::Io(err) => err.fmt(f),
			Error::Meshlet(err) => err.fmt(f),
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

impl From<MeshletError> for Error {
	fn from(value: MeshletError) -> Self {
		Self::Meshlet(value)
	}
}

#[derive(Debug)]
pub enum MeshletError {
	PrimitiveMustBeTriangleList,
	NoVertexPositions,
	NoDefaultScene,
}

impl Display for MeshletError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			MeshletError::PrimitiveMustBeTriangleList => f.write_str("All primitives must be triangle lists"),
			MeshletError::NoVertexPositions => f.write_str("A mesh primitive exists with no vertex positions"),
			MeshletError::NoDefaultScene => f.write_str("No default scene exists"),
		}
	}
}

impl std::error::Error for MeshletError {}

pub type Result<T> = core::result::Result<T, Error>;
