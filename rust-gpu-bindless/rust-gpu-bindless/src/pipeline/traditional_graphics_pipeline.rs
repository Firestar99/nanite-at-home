use crate::descriptor::Bindless;
use crate::pipeline::bindless_pipeline::{BindlessPipeline, VulkanPipeline};
use crate::pipeline::shader::BindlessShader;
use crate::pipeline::specialize::specialize;
use ahash::HashSet;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::{
	FragmentShader, GeometryShader, TesselationControlShader, TesselationEvaluationShader, VertexShader,
};
use smallvec::SmallVec;
use std::sync::Arc;
use vulkano::buffer::{IndexBuffer, Subbuffer};
use vulkano::command_buffer::{DrawIndexedIndirectCommand, DrawIndirectCommand, RecordingCommandBuffer};
use vulkano::pipeline::cache::PipelineCache;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::conservative_rasterization::ConservativeRasterizationState;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::discard_rectangle::DiscardRectangleState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::PipelineSubpassType;
use vulkano::pipeline::graphics::tessellation::TessellationState;
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::{
	DynamicState, GraphicsPipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};
use vulkano::{Validated, ValidationError, VulkanError};

pub type BindlessTraditionalGraphicsPipeline<T> = BindlessPipeline<TraditionalGraphicsPipelineType, T>;

pub struct TraditionalGraphicsPipelineType;

impl VulkanPipeline for TraditionalGraphicsPipelineType {
	type VulkanType = GraphicsPipeline;
	const BINDPOINT: PipelineBindPoint = PipelineBindPoint::Graphics;

	fn bind_pipeline(
		cmd: &mut RecordingCommandBuffer,
		pipeline: Arc<Self::VulkanType>,
	) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>> {
		cmd.bind_pipeline_graphics(pipeline)
	}
}

pub struct TraditionalGraphicsPipelineCreateInfo {
	/// The vertex input state.
	pub vertex_input_state: VertexInputState,

	/// The input assembly state.
	pub input_assembly_state: InputAssemblyState,

	/// The tessellation state.
	pub tessellation_state: Option<TessellationState>,

	/// The viewport state.
	pub viewport_state: ViewportState,

	/// The rasterization state.
	pub rasterization_state: RasterizationState,

	/// The multisample state.
	pub multisample_state: MultisampleState,

	/// The depth/stencil state.
	///
	/// This must be `Some` if `render_pass` has depth/stencil attachments.
	/// It must be `None` otherwise.
	pub depth_stencil_state: Option<DepthStencilState>,

	/// The color blend state.
	///
	/// This must be `Some` if `render_pass` has color attachments.
	/// It must be `None` otherwise.
	pub color_blend_state: Option<ColorBlendState>,

	/// The state(s) that will be set dynamically when recording a command buffer.
	///
	/// The default value is empty.
	pub dynamic_state: HashSet<DynamicState>,

	/// The render subpass to use.
	pub subpass: PipelineSubpassType,

	/// The discard rectangle state.
	///
	/// The default value is `None`.
	pub discard_rectangle_state: Option<DiscardRectangleState>,

	/// The conservative rasterization state.
	///
	/// The default value is `None`.
	pub conservative_rasterization_state: Option<ConservativeRasterizationState>,
}

#[allow(clippy::too_many_arguments)]
impl<T: BufferStruct> BindlessTraditionalGraphicsPipeline<T> {
	fn new(
		stages: SmallVec<[PipelineShaderStageCreateInfo; 5]>,
		create_info: TraditionalGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
		bindless: Arc<Bindless>,
	) -> Result<Self, Validated<VulkanError>> {
		let layout = Self::verify_layout(&bindless, custom_layout)?;
		let device = &bindless.device;
		let ci = GraphicsPipelineCreateInfo {
			flags: Default::default(),
			stages,
			vertex_input_state: Some(create_info.vertex_input_state),
			input_assembly_state: Some(create_info.input_assembly_state),
			tessellation_state: create_info.tessellation_state,
			viewport_state: Some(create_info.viewport_state),
			rasterization_state: Some(create_info.rasterization_state),
			multisample_state: Some(create_info.multisample_state),
			depth_stencil_state: create_info.depth_stencil_state,
			color_blend_state: create_info.color_blend_state,
			dynamic_state: create_info.dynamic_state,
			subpass: Some(create_info.subpass),
			base_pipeline: None,
			discard_rectangle_state: create_info.discard_rectangle_state,
			conservative_rasterization_state: create_info.conservative_rasterization_state,
			..GraphicsPipelineCreateInfo::layout(layout)
		};
		unsafe { Ok(Self::from(GraphicsPipeline::new(device.clone(), cache, ci)?, bindless)) }
	}

	pub fn new_basic(
		bindless: Arc<Bindless>,
		vertex_stage: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: TraditionalGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, vertex_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	pub fn new_geometry(
		bindless: Arc<Bindless>,
		vertex_stage: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		geometry_stage: &impl BindlessShader<ShaderType = GeometryShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: TraditionalGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, vertex_stage)?,
				specialize(&bindless, geometry_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	pub fn new_tesselation(
		bindless: Arc<Bindless>,
		vertex_stage: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		tesselation_control_stage: &impl BindlessShader<ShaderType = TesselationControlShader, ParamConstant = T>,
		tesselation_evaluation_stage: &impl BindlessShader<ShaderType = TesselationEvaluationShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: TraditionalGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, vertex_stage)?,
				specialize(&bindless, tesselation_control_stage)?,
				specialize(&bindless, tesselation_evaluation_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	pub fn new_tesselation_geometry(
		bindless: Arc<Bindless>,
		vertex_stage: &impl BindlessShader<ShaderType = VertexShader, ParamConstant = T>,
		tesselation_control_stage: &impl BindlessShader<ShaderType = TesselationControlShader, ParamConstant = T>,
		tesselation_evaluation_stage: &impl BindlessShader<ShaderType = TesselationEvaluationShader, ParamConstant = T>,
		geometry_stage: &impl BindlessShader<ShaderType = GeometryShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: TraditionalGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, vertex_stage)?,
				specialize(&bindless, tesselation_control_stage)?,
				specialize(&bindless, tesselation_evaluation_stage)?,
				specialize(&bindless, geometry_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	/// Draw a bindless graphics pipeline
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](vulkano::shader#safety) apply.
	pub unsafe fn draw<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		vertex_count: u32,
		instance_count: u32,
		first_vertex: u32,
		first_instance: u32,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.draw(vertex_count, instance_count, first_vertex, first_instance)
		}
	}

	/// Draw a bindless graphics pipeline indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for `DrawIndirectCommand`](DrawIndirectCommand#safety) apply.
	pub unsafe fn draw_indirect<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		indirect_buffer: Subbuffer<[DrawIndirectCommand]>,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe { self.bind_modify(cmd, modify, param)?.draw_indirect(indirect_buffer) }
	}

	/// Draw a bindless graphics pipeline indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for `DrawIndirectCommand`](DrawIndirectCommand#safety) apply.
	/// - The count stored in `count_buffer` must not be greater than the
	///   [`max_draw_indirect_count`](DeviceProperties::max_draw_indirect_count) device limit.
	/// - The count stored in `count_buffer` must fall within the range of `indirect_buffer`.
	pub unsafe fn draw_indirect_count<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		indirect_buffer: Subbuffer<[DrawIndirectCommand]>,
		count_buffer: Subbuffer<u32>,
		max_draw_count: u32,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.draw_indirect_count(indirect_buffer, count_buffer, max_draw_count)
		}
	}

	/// Draw a bindless graphics pipeline
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](vulkano::shader#safety) apply.
	pub unsafe fn draw_indexed<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		index_buffer: impl Into<IndexBuffer>,
		index_count: u32,
		instance_count: u32,
		first_index: u32,
		vertex_offset: i32,
		first_instance: u32,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.bind_index_buffer(index_buffer)?
				.draw_indexed(index_count, instance_count, first_index, vertex_offset, first_instance)
		}
	}

	/// Draw a bindless graphics pipeline indexed indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for `DrawIndirectCommand`](DrawIndirectCommand#safety) apply.
	pub unsafe fn draw_indexed_indirect<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		index_buffer: impl Into<IndexBuffer>,
		indirect_buffer: Subbuffer<[DrawIndexedIndirectCommand]>,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.bind_index_buffer(index_buffer)?
				.draw_indexed_indirect(indirect_buffer)
		}
	}

	/// Draw a bindless graphics pipeline indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for `DrawIndirectCommand`](DrawIndirectCommand#safety) apply.
	/// - The count stored in `count_buffer` must not be greater than the
	///   [`max_draw_indirect_count`](DeviceProperties::max_draw_indirect_count) device limit.
	/// - The count stored in `count_buffer` must fall within the range of `indirect_buffer`.
	pub unsafe fn draw_indexed_indirect_count<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		index_buffer: impl Into<IndexBuffer>,
		indirect_buffer: Subbuffer<[DrawIndexedIndirectCommand]>,
		count_buffer: Subbuffer<u32>,
		max_draw_count: u32,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.bind_index_buffer(index_buffer)?
				.draw_indexed_indirect_count(indirect_buffer, count_buffer, max_draw_count)
		}
	}
}
