use crate::descriptor::Bindless;
use crate::pipeline::bindless_pipeline::{BindlessPipeline, VulkanPipeline};
use crate::pipeline::shader::BindlessShader;
use crate::pipeline::specialize::specialize;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::shader_type::{FragmentShader, MeshShader, TaskShader};
use smallvec::SmallVec;
use std::sync::Arc;

pub type BindlessMeshGraphicsPipeline<T> = BindlessPipeline<MeshGraphicsPipelineType, T>;

pub struct MeshGraphicsPipelineType;

impl VulkanPipeline for MeshGraphicsPipelineType {
	type VulkanType = GraphicsPipeline;
	const BINDPOINT: PipelineBindPoint = PipelineBindPoint::Graphics;

	fn bind_pipeline(
		cmd: &mut RecordingCommandBuffer,
		pipeline: Arc<Self::VulkanType>,
	) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>> {
		cmd.bind_pipeline_graphics(pipeline)
	}
}

pub struct MeshGraphicsPipelineCreateInfo {
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

impl<T: BufferStruct> BindlessMeshGraphicsPipeline<T> {
	fn new(
		stages: SmallVec<[PipelineShaderStageCreateInfo; 5]>,
		create_info: MeshGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
		bindless: Arc<Bindless>,
	) -> Result<Self, Validated<VulkanError>> {
		let layout = Self::verify_layout(&bindless, custom_layout)?;
		let device = &bindless.device;
		let ci = GraphicsPipelineCreateInfo {
			flags: Default::default(),
			stages,
			vertex_input_state: None,
			input_assembly_state: None,
			tessellation_state: None,
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

	pub fn new_mesh(
		bindless: Arc<Bindless>,
		mesh_stage: &impl BindlessShader<ShaderType = MeshShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: MeshGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, mesh_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	pub fn new_task(
		bindless: Arc<Bindless>,
		task_stage: &impl BindlessShader<ShaderType = TaskShader, ParamConstant = T>,
		mesh_stage: &impl BindlessShader<ShaderType = MeshShader, ParamConstant = T>,
		fragment_stage: &impl BindlessShader<ShaderType = FragmentShader, ParamConstant = T>,
		create_info: MeshGraphicsPipelineCreateInfo,
		cache: Option<Arc<PipelineCache>>,
		custom_layout: Option<Arc<PipelineLayout>>,
	) -> Result<Self, Validated<VulkanError>> {
		Self::new(
			SmallVec::from_iter([
				specialize(&bindless, task_stage)?,
				specialize(&bindless, mesh_stage)?,
				specialize(&bindless, fragment_stage)?,
			]),
			create_info,
			cache,
			custom_layout,
			bindless,
		)
	}

	/// Dispatch a bindless mesh graphics pipeline
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](vulkano::shader#safety) apply.
	pub unsafe fn draw_mesh_tasks<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		group_counts: [u32; 3],
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe { self.bind_modify(cmd, modify, param)?.draw_mesh_tasks(group_counts) }
	}

	/// Dispatch a bindless mesh graphics pipeline indirectly
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for
	///   `DrawMeshTasksIndirectCommand`](DrawMeshTasksIndirectCommand#safety) apply.
	pub unsafe fn draw_mesh_tasks_indirect<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		indirect_buffer: Subbuffer<[DrawMeshTasksIndirectCommand]>,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?
				.draw_mesh_tasks_indirect(indirect_buffer)
		}
	}

	/// Dispatch a bindless mesh graphics pipeline indirectly with count
	///
	/// # Safety
	///
	/// - The general [shader safety requirements](crate::shader#safety) apply.
	/// - The [safety requirements for
	///   `DrawMeshTasksIndirectCommand`](DrawMeshTasksIndirectCommand#safety) apply.
	/// - The count stored in `count_buffer` must not be greater than the
	///   [`max_draw_indirect_count`](DeviceProperties::max_draw_indirect_count) device limit.
	/// - The count stored in `count_buffer` must fall within the range of `indirect_buffer`.
	pub unsafe fn draw_mesh_tasks_indirect_count<'a>(
		&self,
		cmd: &'a mut RecordingCommandBuffer,
		indirect_buffer: Subbuffer<[DrawMeshTasksIndirectCommand]>,
		count_buffer: Subbuffer<u32>,
		max_draw_count: u32,
		modify: impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>>,
		param: T,
	) -> Result<&'a mut RecordingCommandBuffer, Box<ValidationError>> {
		unsafe {
			self.bind_modify(cmd, modify, param)?.draw_mesh_tasks_indirect_count(
				indirect_buffer,
				count_buffer,
				max_draw_count,
			)
		}
	}
}
