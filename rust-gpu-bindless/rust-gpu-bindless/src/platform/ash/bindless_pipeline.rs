use crate::descriptor::{Bindless, Format};
use crate::pipeline::graphics_pipeline::GraphicsPipelineCreateInfo;
use crate::pipeline::recording::{Recording, RecordingError};
use crate::pipeline::rendering::RenderPassFormat;
use crate::pipeline::shader::BindlessShader;
use crate::platform::ash::rendering::AshRenderingContext;
use crate::platform::ash::{
	ash_record_and_execute, Ash, AshExecutingContext, AshRecordingContext, AshRecordingError,
	AshRecordingResourceContext, RunOnDrop,
};
use crate::platform::BindlessPipelinePlatform;
use ash::vk::{
	ComputePipelineCreateInfo, DynamicState, Extent2D, Offset2D, Pipeline, PipelineCache,
	PipelineDynamicStateCreateInfo, PipelineMultisampleStateCreateInfo, PipelineRenderingCreateInfo,
	PipelineShaderStageCreateInfo, PipelineTessellationStateCreateInfo, PipelineVertexInputStateCreateInfo,
	PipelineViewportStateCreateInfo, Rect2D, SampleCountFlags, ShaderModuleCreateInfo, ShaderStageFlags, Viewport,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::{ComputeShader, FragmentShader, VertexShader};
use std::sync::Arc;

unsafe impl BindlessPipelinePlatform for Ash {
	type PipelineCreationError = ash::vk::Result;
	type ComputePipeline = AshComputePipeline;
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

	type GraphicsPipeline = AshGraphicsPipeline;
	type MeshGraphicsPipeline = AshMeshGraphicsPipeline;
	type RenderingContext<'a: 'b, 'b> = AshRenderingContext<'a, 'b>;

	unsafe fn create_graphics_pipeline<T: BufferStruct>(
		bindless: &Arc<Bindless<Self>>,
		render_pass: &RenderPassFormat,
		create_info: &GraphicsPipelineCreateInfo,
		vertex_stage: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
	) -> Result<Self::GraphicsPipeline, Self::PipelineCreationError> {
		let device = &bindless.device;
		let vertex_stage = vertex_stage.spirv_binary();
		let vertex_module =
			device.create_shader_module(&ShaderModuleCreateInfo::default().code(vertex_stage.binary), None)?;
		let _vertex_drop = RunOnDrop::new(|| device.destroy_shader_module(vertex_module, None));
		let fragment_stage = fragment_stage.spirv_binary();
		let fragment_module =
			device.create_shader_module(&ShaderModuleCreateInfo::default().code(fragment_stage.binary), None)?;
		let _fragment_drop = RunOnDrop::new(|| device.destroy_shader_module(fragment_module, None));

		let pipelines = device
			.create_graphics_pipelines(
				bindless.cache.unwrap_or(PipelineCache::null()),
				&[ash::vk::GraphicsPipelineCreateInfo::default()
					.layout(bindless.global_descriptor_set().pipeline_layout)
					.stages(&[
						PipelineShaderStageCreateInfo::default()
							.module(vertex_module)
							.stage(ShaderStageFlags::VERTEX)
							.name(vertex_stage.entry_point_name),
						PipelineShaderStageCreateInfo::default()
							.module(fragment_module)
							.stage(ShaderStageFlags::FRAGMENT)
							.name(fragment_stage.entry_point_name),
					])
					.vertex_input_state(&PipelineVertexInputStateCreateInfo::default())
					.input_assembly_state(&create_info.input_assembly_state)
					.tessellation_state(&PipelineTessellationStateCreateInfo::default())
					.viewport_state(
						&PipelineViewportStateCreateInfo::default()
							.viewports(&[Viewport::default()])
							.scissors(&[Rect2D {
								offset: Offset2D { x: 0, y: 0 },
								extent: Extent2D {
									width: i32::MAX as u32,
									height: i32::MAX as u32,
								},
							}]),
					)
					.rasterization_state(&create_info.rasterization_state.line_width(1.0))
					.multisample_state(
						&PipelineMultisampleStateCreateInfo::default().rasterization_samples(SampleCountFlags::TYPE_1),
					)
					.depth_stencil_state(&create_info.depth_stencil_state)
					.color_blend_state(&create_info.color_blend_state)
					.dynamic_state(&PipelineDynamicStateCreateInfo::default().dynamic_states(&[DynamicState::VIEWPORT]))
					.layout(bindless.global_descriptor_set().pipeline_layout)
					.push_next(
						&mut PipelineRenderingCreateInfo::default()
							.color_attachment_formats(render_pass.color_attachments())
							.depth_attachment_format(render_pass.depth_attachment().unwrap_or(Format::default())),
					)],
				None,
			)
			// as we only alloc one pipeline, `e.0.len() == 0` and we don't need to write drop logic
			.map_err(|e| e.1)?;
		Ok(AshGraphicsPipeline(AshPipeline {
			bindless: bindless.clone(),
			pipeline: pipelines[0],
		}))
	}
}

pub struct AshComputePipeline(pub AshPipeline);
pub struct AshGraphicsPipeline(pub AshPipeline);
pub struct AshMeshGraphicsPipeline(pub AshPipeline);

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
