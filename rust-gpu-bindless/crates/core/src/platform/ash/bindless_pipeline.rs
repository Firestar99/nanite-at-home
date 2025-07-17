use crate::descriptor::Bindless;
use crate::pipeline::{
	GraphicsPipelineCreateInfo, MeshGraphicsPipelineCreateInfo, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, Recording, RecordingError,
	RenderPassFormat,
};
use crate::platform::BindlessPipelinePlatform;
use crate::platform::ash::rendering::AshRenderingContext;
use crate::platform::ash::{
	Ash, AshRecordingContext, AshRecordingError, AshRecordingResourceContext, ShaderAshExt, ash_record_and_execute,
};
use ash::prelude::VkResult;
use ash::vk::{
	ComputePipelineCreateInfo, DynamicState, Extent2D, Offset2D, Pipeline, PipelineCache,
	PipelineDynamicStateCreateInfo, PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
	PipelineRenderingCreateInfo, PipelineShaderStageCreateInfo, PipelineTessellationStateCreateInfo,
	PipelineVertexInputStateCreateInfo, PipelineViewportStateCreateInfo, Rect2D, SampleCountFlags, ShaderModule,
	ShaderModuleCreateInfo, Viewport,
};
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader::BindlessShader;
use rust_gpu_bindless_shaders::shader_type::{
	ComputeShader, FragmentShader, MeshShader, ShaderType, TaskShader, VertexShader,
};
use smallvec::SmallVec;
use std::ffi::CStr;
use std::marker::PhantomData;

unsafe impl BindlessPipelinePlatform for Ash {
	type PipelineCreationError = ash::vk::Result;
	type ComputePipeline = AshComputePipeline;
	type RecordingResourceContext = AshRecordingResourceContext;
	type RecordingContext<'a> = AshRecordingContext<'a>;
	type RecordingError = AshRecordingError;

	unsafe fn create_compute_pipeline<T: BufferStruct>(
		bindless: &Bindless<Self>,
		compute_shader: &impl BindlessShader<ShaderType = ComputeShader, ParamConstant = T>,
	) -> Result<Self::ComputePipeline, Self::PipelineCreationError> {
		unsafe {
			let compute = AshShaderModule::new(bindless, compute_shader)?;
			let device = &bindless.device;
			let pipelines = device
				.create_compute_pipelines(
					bindless.cache.unwrap_or(PipelineCache::null()),
					&[ComputePipelineCreateInfo::default()
						.layout(bindless.global_descriptor_set().pipeline_layout)
						.stage(compute.to_shader_stage_create_info())],
					None,
				)
				// as we only alloc one pipeline, `e.0.len() == 0` and we don't need to write drop logic
				.map_err(|e| e.1)?;
			Ok(AshComputePipeline(AshPipeline {
				bindless: bindless.clone(),
				pipeline: pipelines[0],
			}))
		}
	}

	unsafe fn record_and_execute<R: Send + Sync>(
		bindless: &Bindless<Self>,
		f: impl FnOnce(&mut Recording<'_, Self>) -> Result<R, RecordingError<Self>>,
	) -> Result<R, RecordingError<Self>> {
		unsafe { ash_record_and_execute(bindless, f) }
	}

	type GraphicsPipeline = AshGraphicsPipeline;
	type MeshGraphicsPipeline = AshMeshGraphicsPipeline;
	type RenderingContext<'a: 'b, 'b> = AshRenderingContext<'a, 'b>;

	unsafe fn create_graphics_pipeline<T: BufferStruct>(
		bindless: &Bindless<Self>,
		render_pass: &RenderPassFormat,
		create_info: &GraphicsPipelineCreateInfo,
		vertex_shader: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		fragment_shader: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
	) -> Result<Self::GraphicsPipeline, Self::PipelineCreationError> {
		unsafe {
			let vertex = AshShaderModule::new(bindless, vertex_shader)?;
			let fragment = AshShaderModule::new(bindless, fragment_shader)?;
			Ok(AshGraphicsPipeline(Self::ash_create_abstract_graphics_pipeline(
				bindless,
				render_pass,
				create_info.input_assembly_state,
				create_info.rasterization_state,
				create_info.depth_stencil_state,
				create_info.color_blend_state,
				&[
					vertex.to_shader_stage_create_info(),
					fragment.to_shader_stage_create_info(),
				],
			)?))
		}
	}

	unsafe fn create_mesh_graphics_pipeline<T: BufferStruct>(
		bindless: &Bindless<Self>,
		render_pass: &RenderPassFormat,
		create_info: &MeshGraphicsPipelineCreateInfo,
		task_shader: Option<&impl BindlessShader<ShaderType = TaskShader, ParamConstant = T>>,
		mesh_shader: &impl BindlessShader<ShaderType = MeshShader, ParamConstant = T>,
		fragment_shader: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
	) -> Result<Self::MeshGraphicsPipeline, Self::PipelineCreationError> {
		unsafe {
			let task = task_shader
				.map(|task_shader| AshShaderModule::new(bindless, task_shader))
				.transpose()?;
			let mesh = AshShaderModule::new(bindless, mesh_shader)?;
			let fragment = AshShaderModule::new(bindless, fragment_shader)?;
			let stages = [task.as_ref().map(AshShaderModule::to_shader_stage_create_info)]
				.into_iter()
				.flatten()
				.chain([
					mesh.to_shader_stage_create_info(),
					fragment.to_shader_stage_create_info(),
				])
				.collect::<SmallVec<[_; 3]>>();
			Ok(AshMeshGraphicsPipeline(Self::ash_create_abstract_graphics_pipeline(
				bindless,
				render_pass,
				PipelineInputAssemblyStateCreateInfo::default(),
				create_info.rasterization_state,
				create_info.depth_stencil_state,
				create_info.color_blend_state,
				&stages,
			)?))
		}
	}
}

impl Ash {
	#[inline]
	unsafe fn ash_create_abstract_graphics_pipeline(
		bindless: &Bindless<Self>,
		render_pass: &RenderPassFormat,
		input_assembly_state: PipelineInputAssemblyStateCreateInfo,
		rasterization_state: PipelineRasterizationStateCreateInfo,
		depth_stencil_state: PipelineDepthStencilStateCreateInfo,
		color_blend_state: PipelineColorBlendStateCreateInfo,
		stages: &[PipelineShaderStageCreateInfo],
	) -> VkResult<AshPipeline> {
		unsafe {
			let device = &bindless.device;
			let pipelines = device
				.create_graphics_pipelines(
					bindless.cache.unwrap_or(PipelineCache::null()),
					&[ash::vk::GraphicsPipelineCreateInfo::default()
						.layout(bindless.global_descriptor_set().pipeline_layout)
						.stages(stages)
						.vertex_input_state(&PipelineVertexInputStateCreateInfo::default())
						.input_assembly_state(&input_assembly_state)
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
						.rasterization_state(&rasterization_state.line_width(1.0))
						.multisample_state(
							&PipelineMultisampleStateCreateInfo::default()
								.rasterization_samples(SampleCountFlags::TYPE_1),
						)
						.depth_stencil_state(&depth_stencil_state)
						.color_blend_state(&color_blend_state)
						.dynamic_state(
							&PipelineDynamicStateCreateInfo::default()
								.dynamic_states(&[DynamicState::VIEWPORT, DynamicState::SCISSOR]),
						)
						.layout(bindless.global_descriptor_set().pipeline_layout)
						.push_next(
							&mut PipelineRenderingCreateInfo::default()
								.color_attachment_formats(&render_pass.color_attachments)
								.depth_attachment_format(render_pass.depth_attachment.unwrap_or_default()),
						)],
					None,
				)
				// as we only alloc one pipeline, `e.0.len() == 0` and we don't need to write drop logic
				.map_err(|e| e.1)?;
			Ok(AshPipeline {
				bindless: bindless.clone(),
				pipeline: pipelines[0],
			})
		}
	}
}

pub struct AshShaderModule<'a, S: ShaderType, T: BufferStruct> {
	bindless: Bindless<Ash>,
	module: ShaderModule,
	entry_point_name: &'a CStr,
	_phantom: PhantomData<(S, T)>,
}

impl<'a, S: ShaderType, T: BufferStruct> AshShaderModule<'a, S, T> {
	pub fn new(
		bindless: &Bindless<Ash>,
		shader: &'a impl BindlessShader<ShaderType = S, ParamConstant = T>,
	) -> VkResult<Self> {
		unsafe {
			let device = &bindless.device;
			let shader = shader.spirv_binary();
			let module = device.create_shader_module(&ShaderModuleCreateInfo::default().code(shader.binary), None)?;
			bindless.set_debug_object_name(module, &shader.entry_point_name.to_string_lossy())?;
			Ok(Self {
				bindless: bindless.clone(),
				module,
				entry_point_name: shader.entry_point_name,
				_phantom: PhantomData,
			})
		}
	}

	pub fn to_shader_stage_create_info(&self) -> PipelineShaderStageCreateInfo<'_> {
		PipelineShaderStageCreateInfo::default()
			.module(self.module)
			.stage(S::SHADER.to_ash_shader_stage())
			.name(self.entry_point_name)
	}
}

impl<S: ShaderType, T: BufferStruct> Drop for AshShaderModule<'_, S, T> {
	fn drop(&mut self) {
		unsafe { self.bindless.device.destroy_shader_module(self.module, None) }
	}
}

pub struct AshComputePipeline(pub AshPipeline);
pub struct AshGraphicsPipeline(pub AshPipeline);
pub struct AshMeshGraphicsPipeline(pub AshPipeline);

pub struct AshPipeline {
	pub bindless: Bindless<Ash>,
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
