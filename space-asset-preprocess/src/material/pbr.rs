use crate::gltf::Gltf;
use crate::image::image_processor::ImageProcessor;
use glam::{Vec2, Vec3, Vec4};
use gltf::{Material, Primitive};
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

pub fn process_pbr_material<'a>(
	_gltf: &Gltf,
	image_processor: &ImageProcessor<'_>,
	material: Material<'a>,
) -> anyhow::Result<PbrMaterialDisk> {
	profiling::function_scope!();
	let material_id_format = material.index().unwrap_or(!0);
	Ok(PbrMaterialDisk {
		base_color: material.pbr_metallic_roughness().base_color_texture().map(|tex| {
			image_processor.image(
				tex.texture().source(),
				format!("base_color of material {material_id_format}"),
			)
		}),
		base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
		normal: material.normal_texture().map(|tex| {
			image_processor.image(
				tex.texture().source(),
				format!("normal of material {material_id_format}"),
			)
		}),
		normal_scale: material.normal_texture().map_or(1., |n| n.scale()),
		occlusion_roughness_metallic: material
			.pbr_metallic_roughness()
			.metallic_roughness_texture()
			// if metallic_roughness_texture is missing, try to use specular_texture. This fixes Bistro.
			.or_else(|| {
				material
					.specular()
					.and_then(|s| s.specular_texture().or(s.specular_color_texture()))
			})
			.map(|tex| image_processor.image(tex.texture().source(), format!("orm of material {material_id_format}"))),
		occlusion_strength: 0.,
		roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
		metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
	})
}
