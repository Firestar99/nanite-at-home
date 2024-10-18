use crate::descriptor::Bindless;
use crate::pipeline::shader::BindlessShader;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use vulkano::pipeline::PipelineShaderStageCreateInfo;
use vulkano::{Validated, ValidationError, VulkanError};

/// If spec constants are used later on, specialization can be implemented here for all pipelines
pub fn specialize<ShaderType: rust_gpu_bindless_shaders::shader_type::ShaderType, T: BufferStruct>(
	bindless: &Bindless,
	shader: &impl BindlessShader<ShaderType = ShaderType, ParamConstant = T>,
) -> Result<PipelineShaderStageCreateInfo, Validated<VulkanError>> {
	Ok(PipelineShaderStageCreateInfo::new(
		shader
			.load(bindless.device.clone())?
			.single_entry_point()
			.ok_or_else(|| {
				Box::new(ValidationError {
					problem: "Shader does not seem to have an entry point".into(),
					..Default::default()
				})
			})?,
	))
}
