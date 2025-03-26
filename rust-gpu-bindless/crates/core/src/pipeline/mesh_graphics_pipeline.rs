use crate::descriptor::Bindless;
use crate::pipeline::graphics_pipeline::{
	PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo, PipelineRasterizationStateCreateInfo,
};
use crate::pipeline::rendering::RenderPassFormat;
use crate::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader::BindlessShader;
use rust_gpu_bindless_shaders::shader_type::{FragmentShader, MeshShader, TaskShader};
use std::marker::PhantomData;
use std::sync::Arc;

pub struct MeshGraphicsPipelineCreateInfo<'a> {
	pub rasterization_state: PipelineRasterizationStateCreateInfo<'a>,
	pub depth_stencil_state: PipelineDepthStencilStateCreateInfo<'a>,
	pub color_blend_state: PipelineColorBlendStateCreateInfo<'a>,
}

impl<P: BindlessPipelinePlatform> Bindless<P> {
	pub fn create_mesh_graphics_pipeline<T: BufferStruct>(
		&self,
		render_pass: &RenderPassFormat,
		create_info: &MeshGraphicsPipelineCreateInfo,
		task_shader: Option<&impl BindlessShader<ShaderType = TaskShader, ParamConstant = T>>,
		mesh_shader: &impl BindlessShader<ShaderType = MeshShader, ParamConstant = T>,
		fragment_shader: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
	) -> Result<BindlessMeshGraphicsPipeline<P, T>, P::PipelineCreationError> {
		unsafe {
			Ok(BindlessMeshGraphicsPipeline {
				pipeline: Arc::new(P::create_mesh_graphics_pipeline(
					self,
					render_pass,
					create_info,
					task_shader,
					mesh_shader,
					fragment_shader,
				)?),
				_phantom: PhantomData,
			})
		}
	}
}

#[derive(Debug, Clone)]
pub struct BindlessMeshGraphicsPipeline<P: BindlessPipelinePlatform, T: BufferStruct> {
	pipeline: Arc<P::MeshGraphicsPipeline>,
	_phantom: PhantomData<T>,
}

impl<P: BindlessPipelinePlatform, T: BufferStruct> BindlessMeshGraphicsPipeline<P, T> {
	pub fn inner(&self) -> &Arc<P::MeshGraphicsPipeline> {
		&self.pipeline
	}
}
