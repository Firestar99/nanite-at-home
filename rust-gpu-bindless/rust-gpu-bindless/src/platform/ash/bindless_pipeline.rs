use crate::descriptor::Bindless;
use crate::pipeline::recording::{Recording, RecordingError};
use crate::pipeline::shader::BindlessShader;
use crate::platform::ash::{
	ash_record_and_execute, Ash, AshExecutingContext, AshRecordingContext, AshRecordingError,
	AshRecordingResourceContext, RunOnDrop,
};
use crate::platform::BindlessPipelinePlatform;
use ash::vk::{
	ComputePipelineCreateInfo, Pipeline, PipelineCache, PipelineShaderStageCreateInfo, ShaderModuleCreateInfo,
	ShaderStageFlags,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::ComputeShader;
use std::sync::Arc;

unsafe impl BindlessPipelinePlatform for Ash {
	type PipelineCreationError = ash::vk::Result;
	type ComputePipeline = AshComputePipeline;
	type ClassicGraphicsPipeline = AshClassicGraphicsPipeline;
	type MeshGraphicsPipeline = AshMeshhGraphicsPipeline;
	type RecordingResourceContext = AshRecordingResourceContext;
	type RecordingContext<'a> = AshRecordingContext<'a>;
	type RecordingError = AshRecordingError;
	type ExecutingContext<R: Send + Sync> = AshExecutingContext<R>;

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
		Ok(AshComputePipeline(AshPipeline {
			bindless: bindless.clone(),
			pipeline: pipelines[0],
		}))
	}

	unsafe fn record_and_execute<R: Send + Sync>(
		bindless: &Arc<Bindless<Self>>,
		f: impl FnOnce(&mut Recording<'_, Self>) -> Result<R, RecordingError<Self>>,
	) -> Result<Self::ExecutingContext<R>, RecordingError<Self>> {
		ash_record_and_execute(bindless, f)
	}
}

pub struct AshComputePipeline(pub AshPipeline);
pub struct AshClassicGraphicsPipeline(pub AshPipeline);
pub struct AshMeshhGraphicsPipeline(pub AshPipeline);

pub struct AshPipeline {
	pub bindless: Arc<Bindless<Ash>>,
	pub pipeline: Pipeline,
}

impl Drop for AshPipeline {
	fn drop(&mut self) {
		// TODO Pipelines need to be kept alive while executing. Put in TableSync?
		// unsafe {
		// 	self.bindless.device.destroy_pipeline(self.pipeline, None);
		// }
	}
}
