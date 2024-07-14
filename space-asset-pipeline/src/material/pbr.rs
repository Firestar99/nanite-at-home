use crate::gltf::Gltf;
use crate::image::encode::{Encode, EncodeSettings};
use crate::meshlet::error::MeshletError;
use glam::{Vec2, Vec3};
use gltf::{Material, Primitive};
use space_asset::image::ImageType;
use space_asset::material::pbr::vertex::{EncodedPbrVertex, PbrVertex};
use space_asset::material::pbr::PbrMaterialDisk;

#[profiling::function]
pub fn process_pbr_vertices(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<Vec<EncodedPbrVertex>> {
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
	Ok(vertices)
}

#[profiling::function]
pub fn process_pbr_material(
	gltf: &Gltf,
	material: Material,
	settings: EncodeSettings,
) -> anyhow::Result<PbrMaterialDisk> {
	let base_color = material
		.pbr_metallic_roughness()
		.base_color_texture()
		.map(|tex| gltf.image::<{ ImageType::RGBA_COLOR as u32 }>(tex.texture().source()))
		.transpose()?
		.map(|tex| tex.into_optimal_encode(settings))
		.transpose()
		.map_err(|(_, err)| err)?;
	let normal = material
		.normal_texture()
		.map(|tex| gltf.image::<{ ImageType::RG_VALUES as u32 }>(tex.texture().source()))
		.transpose()?
		.map(|tex| tex.into_optimal_encode(settings))
		.transpose()
		.map_err(|(_, err)| err)?;
	let omr = material
		.pbr_metallic_roughness()
		.metallic_roughness_texture()
		.map(|tex| gltf.image::<{ ImageType::RGBA_LINEAR as u32 }>(tex.texture().source()))
		.transpose()?
		.map(|tex| tex.into_optimal_encode(settings))
		.transpose()
		.map_err(|(_, err)| err)?;

	Ok(PbrMaterialDisk {
		base_color,
		base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
		normal,
		normal_scale: material.normal_texture().map_or(1., |n| n.scale()),
		omr,
		occlusion_strength: 0.,
		roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
		metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
	})
}
