use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Desc, DescRef, Image, Image2d};

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct PbrMaterial<R: DescRef> {
	pub base_color: Desc<R, Image<Image2d>>,
	pub base_color_factor: [f32; 4],
	pub normal: Desc<R, Image<Image2d>>,
	pub normal_scale: f32,
	pub occlusion_roughness_metallic: Desc<R, Image<Image2d>>,
	pub occlusion_strength: f32,
	pub metallic_factor: f32,
	pub roughness_factor: f32,
}

pub use space_asset_disk_shader::material::pbr::*;
