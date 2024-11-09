use crate::image::upload::upload_image2d_archive;
use crate::upload_traits::ToStrong;
use crate::uploader::{UploadError, Uploader};
use space_asset_disk::material::pbr::ArchivedPbrMaterialDisk;
use space_asset_shader::material::pbr::PbrMaterial;
use std::future::Future;
use vulkano::Validated;
use vulkano_bindless::descriptor::{RCDescExt, RC};
use vulkano_bindless_shaders::descriptor::Strong;

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

pub fn upload_pbr_material<'a>(
	this: &'a ArchivedPbrMaterialDisk,
	uploader: &'a Uploader,
) -> impl Future<Output = Result<PbrMaterial<RC>, Validated<UploadError>>> + 'a {
	let base_color = this
		.base_color
		.as_ref()
		.map(|tex| upload_image2d_archive(tex, uploader));
	let normal = this.normal.as_ref().map(|tex| upload_image2d_archive(tex, uploader));
	let occlusion_roughness_metallic = this
		.occlusion_roughness_metallic
		.as_ref()
		.map(|tex| upload_image2d_archive(tex, uploader));
	async {
		Ok(PbrMaterial {
			base_color: uploader.await_or_white_texture(base_color).await?,
			base_color_factor: this.base_color_factor,
			normal: uploader.await_or_white_texture(normal).await?,
			normal_scale: this.normal_scale,
			occlusion_roughness_metallic: uploader.await_or_white_texture(occlusion_roughness_metallic).await?,
			occlusion_strength: this.occlusion_strength,
			metallic_factor: this.metallic_factor,
			roughness_factor: this.roughness_factor,
		})
	}
}

pub fn default_pbr_material(uploader: &Uploader) -> PbrMaterial<RC> {
	PbrMaterial {
		base_color: uploader.white_texture(),
		base_color_factor: [1.; 4],
		normal: uploader.white_texture(),
		normal_scale: 1.,
		occlusion_roughness_metallic: uploader.white_texture(),
		occlusion_strength: 1.,
		metallic_factor: 1.,
		roughness_factor: 1.,
	}
}
