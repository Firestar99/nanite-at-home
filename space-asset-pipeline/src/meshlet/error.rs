use std::fmt::{Debug, Display, Formatter};

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
