use crate::gltf::Gltf;
use crate::image::image_processor::{ImageAccessor, ImageProcessor, RequestedImage};
use glam::{Vec2, Vec3, Vec4};
use gltf::{Material, Primitive};
use space_asset_disk::image::ImageType;
use space_asset_disk::material::pbr::PbrMaterialDisk;
use space_asset_disk::material::pbr::PbrVertex;

pub fn process_pbr_vertices(gltf: &Gltf, primitive: Primitive) -> anyhow::Result<Vec<PbrVertex>> {
	profiling::function_scope!();
	let reader = primitive.reader(|b| gltf.buffer(b));
	let vertex_cnt = reader.read_positions().unwrap().len();
	let mut tex_coords = reader.read_tex_coords(0).map(|tex| tex.into_f32());
	let mut normals = reader.read_normals();
	let mut tangents = reader.read_tangents();
	let vertices = (0..vertex_cnt)
		.map(|_| PbrVertex {
			normal: normals.as_mut().and_then(|n| n.next()).map_or(Vec3::ZERO, Vec3::from),
			tangent: tangents.as_mut().and_then(|n| n.next()).map_or(Vec4::ZERO, Vec4::from),
			tex_coord: tex_coords
				.as_mut()
				.and_then(|tex| tex.next())
				.map_or(Vec2::ZERO, Vec2::from),
		})
		.collect();
	Ok(vertices)
}

pub struct ProcessedPbrMaterial<'a> {
	material: Material<'a>,
	base_color: Option<RequestedImage<{ ImageType::RGBA_COLOR as u32 }>>,
	normal: Option<RequestedImage<{ ImageType::RG_VALUES as u32 }>>,
	occlusion_roughness_metallic: Option<RequestedImage<{ ImageType::RGBA_LINEAR as u32 }>>,
}

pub fn process_pbr_material<'a>(
	_gltf: &Gltf,
	image_processor: &ImageProcessor<'_>,
	material: Material<'a>,
) -> anyhow::Result<ProcessedPbrMaterial<'a>> {
	profiling::function_scope!();
	Ok(ProcessedPbrMaterial {
		base_color: material
			.pbr_metallic_roughness()
			.base_color_texture()
			.map(|tex| image_processor.image::<{ ImageType::RGBA_COLOR as u32 }>(tex.texture().source())),
		normal: material
			.normal_texture()
			.map(|tex| image_processor.image::<{ ImageType::RG_VALUES as u32 }>(tex.texture().source())),
		occlusion_roughness_metallic: material
			.pbr_metallic_roughness()
			.metallic_roughness_texture()
			// if metallic_roughness_texture is missing, try to use specular_texture. This fixes Bistro.
			.or_else(|| {
				material
					.specular()
					.and_then(|s| s.specular_texture().or(s.specular_color_texture()))
			})
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
			occlusion_roughness_metallic: self.occlusion_roughness_metallic.map(|tex| tex.get(image_accessor)),
			occlusion_strength: 0.,
			roughness_factor: self.material.pbr_metallic_roughness().roughness_factor(),
			metallic_factor: self.material.pbr_metallic_roughness().metallic_factor(),
		})
	}
}
