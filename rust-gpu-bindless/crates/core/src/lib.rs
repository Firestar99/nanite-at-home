pub mod backing;
pub mod descriptor;
pub mod pipeline;
pub mod platform;

pub mod __private {
	pub use ash::vk::make_api_version;
	pub use rust_gpu_bindless_shaders::__private::*;
	pub use rust_gpu_bindless_shaders::{shader, shader_type};
}
