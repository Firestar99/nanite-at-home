use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ShaderType;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::shader::ShaderModule;
use vulkano::{Validated, VulkanError};

pub trait BindlessShader {
	type ShaderType: ShaderType;
	type ParamConstant: BufferStruct;

	fn load(&self, device: Arc<Device>) -> Result<Arc<ShaderModule>, Validated<VulkanError>>;
}
