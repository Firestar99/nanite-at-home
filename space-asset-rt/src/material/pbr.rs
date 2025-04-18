use crate::image::upload::UploadedImages;
use crate::upload_traits::ToStrong;
use rust_gpu_bindless::descriptor::{RCDescExt, RC};
use rust_gpu_bindless_shaders::descriptor::Strong;
use space_asset_disk::material::pbr::ArchivedPbrMaterialDisk;
use space_asset_shader::material::pbr::PbrMaterial;

pub struct PbrMaterials<'a> {
	pub pbr_materials: &'a [PbrMaterial<RC>],
	pub default_pbr_material: &'a PbrMaterial<RC>,
}

impl ToStrong for PbrMaterial<RC> {
	type StrongType = PbrMaterial<Strong>;

	fn to_strong(&self) -> Self::StrongType {
		PbrMaterial {
			base_color: self.base_color.to_strong(),
			base_color_factor: self.base_color_factor,
			normal: self.normal.to_strong(),
			normal_scale: self.normal_scale,
			occlusion_roughness_metallic: self.occlusion_roughness_metallic.to_strong(),
			occlusion_strength: self.occlusion_strength,
			metallic_factor: self.metallic_factor,
			roughness_factor: self.roughness_factor,
		}
	}
}

pub fn upload_pbr_material(
	this: &ArchivedPbrMaterialDisk,
	uploader: &UploadedImages,
) -> anyhow::Result<PbrMaterial<RC>> {
	profiling::scope!("upload_pbr_material");
	Ok(PbrMaterial {
		base_color: this
			.base_color
			.as_ref()
			.map(|tex| uploader.archived_image(tex))
			.unwrap_or(&uploader.default_white_texture)
			.clone(),
		base_color_factor: this.base_color_factor.map(|i| i.to_native()),
		normal: this
			.normal
			.as_ref()
			.map(|tex| uploader.archived_image(tex))
			.unwrap_or(&uploader.default_normal_texture)
			.clone(),
		normal_scale: this.normal_scale.to_native(),
		occlusion_roughness_metallic: this
			.occlusion_roughness_metallic
			.as_ref()
			.map(|tex| uploader.archived_image(tex))
			.unwrap_or(&uploader.default_white_texture)
			.clone(),
		occlusion_strength: this.occlusion_strength.to_native(),
		metallic_factor: this.metallic_factor.to_native(),
		roughness_factor: this.roughness_factor.to_native(),
	})
}

pub fn default_pbr_material(uploader: &UploadedImages) -> PbrMaterial<RC> {
	PbrMaterial {
		base_color: uploader.default_white_texture.clone(),
		base_color_factor: [1.; 4],
		normal: uploader.default_normal_texture.clone(),
		normal_scale: 1.,
		occlusion_roughness_metallic: uploader.default_white_texture.clone(),
		occlusion_strength: 1.,
		metallic_factor: 1.,
		roughness_factor: 1.,
	}
}
