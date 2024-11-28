use crate::descriptor::mutable::MutDescExt;
use crate::descriptor::{Bindless, BindlessFrame};
use crate::pipeline::shader::BindlessShader;
use crate::platform::ash::{Ash, AshExecutingCommandBuffer, AshRecordingCommandBuffer, RunOnDrop};
use crate::platform::BindlessPipelinePlatform;
use ash::vk::{
	ComputePipelineCreateInfo, PipelineCache, PipelineShaderStageCreateInfo, ShaderModuleCreateInfo, ShaderStageFlags,
};
use rust_gpu_bindless_shaders::buffer_content::{BufferStruct, Metadata};
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::sync::Arc;

unsafe impl BindlessPipelinePlatform for Ash {
	type PipelineCreationError = ash::vk::Result;
	type ComputePipeline = ash::vk::Pipeline;
	type TraditionalGraphicsPipeline = ash::vk::Pipeline;
	type MeshGraphicsPipeline = ash::vk::Pipeline;
	type RecordingCommandBuffer<'a> = AshRecordingCommandBuffer<'a>;
	type RecordingError = ash::vk::Result;
	type ExecutingCommandBuffer = AshExecutingCommandBuffer;

	unsafe fn create_compute_pipeline<T: BufferStruct>(
		bindless: &Arc<Bindless<Self>>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self::ComputePipeline, Self::PipelineCreationError> {
		let compute_shader = compute_shader.spirv_binary();
		let device = &bindless.device;
		let module =
			device.create_shader_module(&ShaderModuleCreateInfo::default().code(compute_shader.binary), None)?;
		let _module_drop = RunOnDrop::new(|| device.destroy_shader_module(module, None));

		let pipelines = device
			.create_compute_pipelines(
				bindless.cache.unwrap_or(PipelineCache::null()),
				&[ComputePipelineCreateInfo::default()
					.layout(bindless.global_descriptor_set().pipeline_layout)
					.stage(
						PipelineShaderStageCreateInfo::default()
							.module(module)
							.stage(ShaderStageFlags::COMPUTE)
							.name(compute_shader.entry_point_name),
					)],
				None,
			)
			// as we only alloc one pipeline, `e.0.len() == 0` and we don't need to write drop logic
			.map_err(|e| e.1)?;
		Ok(pipelines[0])
	}

	unsafe fn start_recording(
		bindless_frame: &Arc<BindlessFrame<Self>>,
	) -> Result<Self::RecordingCommandBuffer, Self::RecordingError> {
		Ok(AshRecordingCommandBuffer::new(
			bindless_frame.bindless.platform.execution_manager.pop(),
			metadata,
		))
	}
}
