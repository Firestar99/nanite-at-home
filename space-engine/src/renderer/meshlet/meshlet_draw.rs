use crate::renderer::compacting_alloc_buffer::CompactingAllocBufferReading;
use crate::renderer::frame_context::FrameContext;
use ash::vk::{ColorComponentFlags, CompareOp, Filter, PipelineColorBlendAttachmentState, SamplerCreateInfo};
use rust_gpu_bindless::descriptor::{Bindless, RCDesc, RCDescExt, Sampler};
use rust_gpu_bindless::pipeline::{
	BindlessMeshGraphicsPipeline, MeshGraphicsPipelineCreateInfo, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineRasterizationStateCreateInfo, RecordingError, RenderPassFormat,
	Rendering,
};
use rust_gpu_bindless_shaders::shader::{BindlessShader, SpirvBinary};
use rust_gpu_bindless_shaders::shader_type::TaskShader;
use space_asset_rt::meshlet::scene::InstancedMeshletSceneCpu;
use space_engine_shader::renderer::meshlet::intermediate::MeshletInstance;
use space_engine_shader::renderer::meshlet::mesh_shader::Param;

pub struct MeshletDraw {
	pipeline: BindlessMeshGraphicsPipeline<Param<'static>>,
	sampler: RCDesc<Sampler>,
}

impl MeshletDraw {
	pub fn new(bindless: &Bindless, g_buffer_format: RenderPassFormat) -> anyhow::Result<Self> {
		let pipeline = bindless.create_mesh_graphics_pipeline::<Param<'static>>(
			&g_buffer_format,
			&MeshGraphicsPipelineCreateInfo {
				rasterization_state: PipelineRasterizationStateCreateInfo::default().line_width(1.),
				color_blend_state: PipelineColorBlendStateCreateInfo::default().attachments(&[
					PipelineColorBlendAttachmentState::default().color_write_mask(ColorComponentFlags::RGBA),
					PipelineColorBlendAttachmentState::default().color_write_mask(ColorComponentFlags::RGBA),
					PipelineColorBlendAttachmentState::default().color_write_mask(ColorComponentFlags::RGBA),
				]),
				depth_stencil_state: PipelineDepthStencilStateCreateInfo::default()
					.depth_test_enable(true)
					.depth_write_enable(true)
					.depth_compare_op(CompareOp::LESS),
			},
			Option::<&FakeTaskShader>::None,
			crate::shader::renderer::meshlet::mesh_shader::meshlet_mesh::new(),
			crate::shader::renderer::meshlet::mesh_shader::meshlet_fragment_g_buffer::new(),
		)?;

		let sampler = bindless.sampler().alloc_ash(
			&SamplerCreateInfo::default()
				.mag_filter(Filter::LINEAR)
				.min_filter(Filter::LINEAR),
		)?;

		Ok(Self { pipeline, sampler })
	}

	pub fn draw(
		&self,
		cmd: &mut Rendering,
		frame_context: &FrameContext,
		scene: &InstancedMeshletSceneCpu,
		alloc_buffer: &CompactingAllocBufferReading<MeshletInstance>,
	) -> Result<(), RecordingError> {
		profiling::function_scope!();
		let param = Param {
			frame_data: frame_context.frame_data_desc,
			scene: scene.scene.to_transient(cmd),
			sampler: self.sampler.to_transient(cmd),
			compacting_alloc_buffer: alloc_buffer.to_reader()?,
		};
		cmd.draw_mesh_tasks_indirect(&self.pipeline, alloc_buffer.indirect_args(), param)
	}
}

pub enum FakeTaskShader {}

impl BindlessShader for FakeTaskShader {
	type ShaderType = TaskShader;
	type ParamConstant = Param<'static>;

	fn spirv_binary(&self) -> &SpirvBinary {
		unreachable!()
	}
}
