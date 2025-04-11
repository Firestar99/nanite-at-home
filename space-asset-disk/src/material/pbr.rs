use crate::image::{ImageDiskRgLinear, ImageDiskRgbaLinear, ImageDiskRgbaSrgb};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct PbrMaterialDisk {
	pub base_color: Option<ImageDiskRgbaSrgb>,
	pub base_color_factor: [f32; 4],
	pub normal: Option<ImageDiskRgLinear>,
	pub normal_scale: f32,
	pub occlusion_roughness_metallic: Option<ImageDiskRgbaLinear>,
	pub occlusion_strength: f32,
	pub metallic_factor: f32,
	pub roughness_factor: f32,
}

pub use space_asset_disk_shader::material::pbr::*;
