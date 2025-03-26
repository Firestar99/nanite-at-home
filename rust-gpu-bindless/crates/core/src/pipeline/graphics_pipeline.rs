use crate::descriptor::Bindless;
use crate::pipeline::rendering::RenderPassFormat;
use crate::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader::BindlessShader;
use rust_gpu_bindless_shaders::shader_type::{FragmentShader, VertexShader};
use std::marker::PhantomData;
use std::sync::Arc;

pub type PipelineInputAssemblyStateCreateInfo<'a> = ash::vk::PipelineInputAssemblyStateCreateInfo<'a>;
pub type PipelineRasterizationStateCreateInfo<'a> = ash::vk::PipelineRasterizationStateCreateInfo<'a>;
pub type PipelineDepthStencilStateCreateInfo<'a> = ash::vk::PipelineDepthStencilStateCreateInfo<'a>;
pub type PipelineColorBlendStateCreateInfo<'a> = ash::vk::PipelineColorBlendStateCreateInfo<'a>;

pub struct GraphicsPipelineCreateInfo<'a> {
	// pub ia_topology: PrimitiveTopology,
	// pub ia_primitive_restart_enable: bool,
	// pub raster_polygon_mode: PolygonMode,
	// pub raster_cull_mode: CullModeFlags,
	// pub raster_front_face: FrontFace,

	// pub vertex_input_state: PipelineVertexInputStateCreateInfo<'a>,
	pub input_assembly_state: PipelineInputAssemblyStateCreateInfo<'a>,
	// pub tessellation_state: Option<PipelineTessellationStateCreateInfo<'a>>,
	// pub viewport_state: PipelineViewportStateCreateInfo<'a>,
	pub rasterization_state: PipelineRasterizationStateCreateInfo<'a>,
	// pub multisample_state: PipelineMultisampleStateCreateInfo<'a>,
	pub depth_stencil_state: PipelineDepthStencilStateCreateInfo<'a>,
	pub color_blend_state: PipelineColorBlendStateCreateInfo<'a>,
}

impl<P: BindlessPipelinePlatform> Bindless<P> {
	pub fn create_graphics_pipeline<T: BufferStruct>(
		&self,
		render_pass: &RenderPassFormat,
		create_info: &GraphicsPipelineCreateInfo,
		vertex_shader: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		fragment_shader: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
	) -> Result<BindlessGraphicsPipeline<P, T>, P::PipelineCreationError> {
		unsafe {
			Ok(BindlessGraphicsPipeline {
				pipeline: Arc::new(P::create_graphics_pipeline(
					self,
					render_pass,
					create_info,
					vertex_shader,
					fragment_shader,
				)?),
				_phantom: PhantomData,
			})
		}
	}
}

#[derive(Debug, Clone)]
pub struct BindlessGraphicsPipeline<P: BindlessPipelinePlatform, T: BufferStruct> {
	pipeline: Arc<P::GraphicsPipeline>,
	_phantom: PhantomData<T>,
}

impl<P: BindlessPipelinePlatform, T: BufferStruct> BindlessGraphicsPipeline<P, T> {
	pub fn inner(&self) -> &Arc<P::GraphicsPipeline> {
		&self.pipeline
	}
}
