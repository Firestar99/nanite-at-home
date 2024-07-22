use crate::gltf::Gltf;
use crate::image::image_processor::{ImageAccessor, ImageProcessor, RequestedImage};
use crate::meshlet::error::MeshletError;
use glam::{Vec2, Vec3};
use gltf::{Material, Primitive};
use space_asset_disk::image::ImageType;
use space_asset_disk::material::pbr::vertex::PbrVertex;
use space_asset_disk::material::pbr::PbrMaterialDisk;

#[profiling::function]
pub fn process_pbr_vertices(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<Vec<PbrVertex>> {
	let reader = primitive.reader(|b| gltf.buffer(b));
	let vertices = reader
		.read_tex_coords(0)
		.ok_or(MeshletError::NoTextureCoords)?
		.into_f32()
		.zip(reader.read_normals().ok_or(MeshletError::NoNormals)?)
		.map(|(tex_coords, normals)| PbrVertex {
			normals: Vec3::from(normals),
			tex_coords: Vec2::from(tex_coords),
		})
		.collect();
	Ok(vertices)
}

pub struct ProcessedPbrMaterial<'a> {
	material: Material<'a>,
	base_color: Option<RequestedImage<{ ImageType::RGBA_COLOR as u32 }>>,
	normal: Option<RequestedImage<{ ImageType::RG_VALUES as u32 }>>,
	omr: Option<RequestedImage<{ ImageType::RGBA_LINEAR as u32 }>>,
}

#[profiling::function]
pub fn process_pbr_material<'a>(
	_gltf: &Gltf,
	image_processor: &ImageProcessor<'_>,
	material: Material<'a>,
) -> anyhow::Result<ProcessedPbrMaterial<'a>> {
	Ok(ProcessedPbrMaterial {
		base_color: material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|tex| image_processor.image::<{ ImageType::RGBA_COLOR as u32 }>(tex.texture().source())),
		normal: material
			.normal_texture()
			.map(|tex| image_processor.image::<{ ImageType::RG_VALUES as u32 }>(tex.texture().source())),
		omr: material
			.pbr_metallic_roughness()
			.metallic_roughness_texture()
			.map(|tex| image_processor.image::<{ ImageType::RGBA_LINEAR as u32 }>(tex.texture().source())),
		material,
	})
}

impl<'a> ProcessedPbrMaterial<'a> {
	pub fn finish(self, image_accessor: &ImageAccessor) -> anyhow::Result<PbrMaterialDisk> {
		Ok(PbrMaterialDisk {
			base_color: self.base_color.map(|tex| tex.get(image_accessor)),
			base_color_factor: self.material.pbr_metallic_roughness().base_color_factor(),
			normal: self.normal.map(|tex| tex.get(image_accessor)),
			normal_scale: self.material.normal_texture().map_or(1., |n| n.scale()),
			omr: self.omr.map(|tex| tex.get(image_accessor)),
			occlusion_strength: 0.,
			roughness_factor: self.material.pbr_metallic_roughness().roughness_factor(),
			metallic_factor: self.material.pbr_metallic_roughness().metallic_factor(),
		})
	}
}
