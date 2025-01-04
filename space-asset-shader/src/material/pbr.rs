use vulkano_bindless_macros::BufferContent;
use vulkano_bindless_shaders::descriptor::{Desc, DescRef};
use vulkano_bindless_shaders::spirv_std::image::Image2d;

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferContent)]
pub struct PbrMaterial<R: DescRef> {
	pub base_color: Desc<R, Image2d>,
	pub base_color_factor: [f32; 4],
	pub normal: Desc<R, Image2d>,
	pub normal_scale: f32,
	pub occlusion_roughness_metallic: Desc<R, Image2d>,
	pub occlusion_strength: f32,
	pub metallic_factor: f32,
	pub roughness_factor: f32,
}

pub use space_asset_disk_shader::material::pbr::*;
