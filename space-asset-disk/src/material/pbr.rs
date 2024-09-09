use crate::image::{Image2DDisk, ImageType};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct PbrMaterialDisk {
	pub base_color: Option<Image2DDisk<{ ImageType::RGBA_COLOR as u32 }>>,
	pub base_color_factor: [f32; 4],
	pub normal: Option<Image2DDisk<{ ImageType::RG_VALUES as u32 }>>,
	pub normal_scale: f32,
	pub omr: Option<Image2DDisk<{ ImageType::RGBA_LINEAR as u32 }>>,
	pub occlusion_strength: f32,
	pub metallic_factor: f32,
	pub roughness_factor: f32,
}

pub use space_asset_disk_shader::material::pbr::*;
