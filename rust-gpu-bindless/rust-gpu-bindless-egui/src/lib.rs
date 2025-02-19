use rust_gpu_bindless::generic::platform::BindlessPipelinePlatform;

pub mod ash;
pub(crate) mod shaders;
pub mod winit_integration;

pub unsafe trait EguiBindlessPlatform: BindlessPipelinePlatform {
	unsafe fn max_image_dimensions_2d(&self) -> u32;
}
