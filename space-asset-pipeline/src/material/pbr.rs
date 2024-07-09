use crate::image::ImageExt;
use crate::meshlet::error::{Error, MeshletError};
use crate::meshlet::process::Gltf;
use glam::{Vec2, Vec3};
use gltf::material::NormalTexture;
use gltf::texture::Info;
use gltf::Primitive;
use rayon::prelude::*;
use space_asset::image::Image2DDisk;
use space_asset::material::pbr::vertex::PbrVertex;
use space_asset::material::pbr::PbrMaterialDisk;
use std::sync::Arc;

pub enum TextureType<'a> {
	Normal(NormalTexture<'a>),
	Rgba(Info<'a>),
}

impl<'a> From<NormalTexture<'a>> for TextureType<'a> {
	fn from(value: NormalTexture<'a>) -> Self {
		Self::Normal(value)
	}
}

impl<'a> From<Info<'a>> for TextureType<'a> {
	fn from(value: Info<'a>) -> Self {
		Self::Rgba(value)
	}
}

pub fn process_pbr_material(gltf: &Arc<Gltf>, primitive: Primitive) -> crate::meshlet::error::Result<PbrMaterialDisk> {
	let reader = primitive.reader(|b| gltf.buffer(b));
	let vertices = reader
		.read_tex_coords(0)
		.ok_or(MeshletError::NoTextureCoords)?
		.into_f32()
		.zip(reader.read_normals().ok_or(MeshletError::NoNormals)?)
		.map(|(tex_coords, normals)| {
			PbrVertex {
				normals: Vec3::from(normals),
				tex_coords: Vec2::from(tex_coords),
			}
			.encode()
		})
		.collect();

	let material = primitive.material();
	let textures = [
		material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(TextureType::from),
		material.normal_texture().map(TextureType::from),
		material
			.pbr_metallic_roughness()
			.metallic_roughness_texture()
			.map(TextureType::from),
	];

	let mut textures = textures
		.par_iter()
		.map(|tex| {
			if let Some(tex) = tex {
				match tex {
					TextureType::Normal(tex) => {
						if tex.tex_coord() != 0 {
							return Err(Error::from(MeshletError::MultipleTextureCoords));
						}
						let image = gltf.image(tex.texture().source()).map_err(Error::from)?;
						Ok(Some(Image2DDisk::encode_normal_map(image)))
					}
					TextureType::Rgba(tex) => {
						if tex.tex_coord() != 0 {
							return Err(Error::from(MeshletError::MultipleTextureCoords));
						}
						let image = gltf.image(tex.texture().source()).map_err(Error::from)?;
						Ok(Some(Image2DDisk::encode_rgba(image)))
					}
				}
			} else {
				Ok(None)
			}
		})
		.collect::<crate::meshlet::error::Result<Vec<_>>>()?;

	Ok(PbrMaterialDisk {
		vertices,
		base_color: textures.get_mut(0).unwrap().take(),
		base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
		normal: textures.get_mut(1).unwrap().take(),
		normal_scale: material.normal_texture().map_or(1., |n| n.scale()),
		omr: textures.get_mut(2).unwrap().take(),
		occlusion_strength: 0.,
		roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
		metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
	})
}
