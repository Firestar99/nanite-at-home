use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::{Desc, DescRef, Image, Image2d};

#[derive(Copy, Clone, BufferStruct)]
pub struct GBuffer<R: DescRef> {
	pub g_albedo: Desc<R, Image<Image2d>>,
	pub g_normal: Desc<R, Image<Image2d>>,
	pub g_roughness_metallic: Desc<R, Image<Image2d>>,
	pub depth_image: Desc<R, Image<Image2d>>,
}
