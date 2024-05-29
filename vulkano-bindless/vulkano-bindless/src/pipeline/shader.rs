use std::sync::Arc;
use vulkano::device::Device;
use vulkano::shader::ShaderModule;
use vulkano::{Validated, VulkanError};
use vulkano_bindless_shaders::desc_buffer::DescStruct;
use vulkano_bindless_shaders::shader_type::ShaderType;

pub trait BindlessShader {
	type ShaderType: ShaderType;
	type ParamConstant: DescStruct;

	fn load(&self, device: Arc<Device>) -> Result<Arc<ShaderModule>, Validated<VulkanError>>;
}
