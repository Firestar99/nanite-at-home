use rust_gpu_bindless::generic::platform::BindlessPipelinePlatform;

pub mod ash;

pub unsafe trait EguiBindlessPlatform: BindlessPipelinePlatform {
	unsafe fn max_image_dimensions_2d(&self) -> u32;
}
