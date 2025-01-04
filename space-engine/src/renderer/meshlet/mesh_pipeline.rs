use crate::renderer::meshlet::meshlet_allocation_buffer::MeshletAllocationBuffer;
use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use space_asset_rt::meshlet::scene::MeshletSceneCpu;
use space_engine_shader::renderer::meshlet::mesh_shader::Params;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::format::Format;
use vulkano::image::sampler::SamplerCreateInfo;
use vulkano::pipeline::graphics::color_blend::{
	AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState, ColorComponents,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::{DynamicState, Pipeline, PipelineBindPoint, PipelineLayout};
use vulkano_bindless::descriptor::{RCDesc, RCDescExt, Sampler};
use vulkano_bindless::pipeline::mesh_graphics_pipeline::{
	BindlessMeshGraphicsPipeline, MeshGraphicsPipelineCreateInfo,
};

pub struct MeshDrawPipeline {
	pipeline: BindlessMeshGraphicsPipeline<Params<'static>>,
	sampler: RCDesc<Sampler>,
}

impl MeshDrawPipeline {
	pub fn new(
		init: &Arc<Init>,
		alloc_buffer: &MeshletAllocationBuffer,
		g_albedo_format_srgb: Format,
		g_normal_format: Format,
		g_roughness_metallic_format: Format,
		depth_format: Format,
	) -> Self {
		let pipeline = BindlessMeshGraphicsPipeline::new_mesh(
			init.bindless.clone(),
			crate::shader::renderer::meshlet::mesh_shader::meshlet_mesh::new(),
			crate::shader::renderer::meshlet::mesh_shader::meshlet_fragment_g_buffer::new(),
			MeshGraphicsPipelineCreateInfo {
				viewport_state: ViewportState::default(),
				rasterization_state: RasterizationState::default(),
				multisample_state: MultisampleState::default(),
				depth_stencil_state: Some(DepthStencilState {
					depth: Some(DepthState {
						write_enable: true,
						compare_op: CompareOp::Less,
					}),
					..DepthStencilState::default()
				}),
				color_blend_state: Some(ColorBlendState {
					attachments: vec![
						ColorBlendAttachmentState {
							blend: Some(AttachmentBlend {
								src_color_blend_factor: BlendFactor::One,
								dst_color_blend_factor: BlendFactor::Zero,
								color_blend_op: BlendOp::Add,
								src_alpha_blend_factor: BlendFactor::One,
								dst_alpha_blend_factor: BlendFactor::Zero,
								alpha_blend_op: BlendOp::Add,
							}),
							color_write_enable: true,
							color_write_mask: ColorComponents::all(),
						};
						3
					],
					..Default::default()
				}),
				subpass: PipelineSubpassType::BeginRendering(PipelineRenderingCreateInfo {
					color_attachment_formats: vec![
						Some(g_albedo_format_srgb),
						Some(g_normal_format),
						Some(g_roughness_metallic_format),
					],
					depth_attachment_format: Some(depth_format),
					..PipelineRenderingCreateInfo::default()
				}),
				discard_rectangle_state: None,
				dynamic_state: [DynamicState::Viewport].into_iter().collect(),
				conservative_rasterization_state: None,
			},
			Some(init.pipeline_cache.deref().clone()),
			Some(
				PipelineLayout::new(
					init.device.clone(),
					PipelineLayoutCreateInfo {
						set_layouts: Vec::from([
							init.bindless.descriptor_set_layout.clone(),
							alloc_buffer.descriptor_set.layout().clone(),
						]),
						push_constant_ranges: init.bindless.get_push_constant::<Params<'static>>(),
						..PipelineLayoutCreateInfo::default()
					},
				)
				.unwrap(),
			),
		)
		.unwrap();

		let sampler = init
			.bindless
			.sampler()
			.alloc(SamplerCreateInfo::simple_repeat_linear())
			.unwrap();

		Self { pipeline, sampler }
	}

	#[profiling::function]
	pub fn draw(
		&self,
		frame_context: &FrameContext,
		cmd: &mut RecordingCommandBuffer,
		alloc_buffer: &MeshletAllocationBuffer,
		scene: &MeshletSceneCpu,
	) {
		unsafe {
			self.pipeline
				.draw_mesh_tasks_indirect(
					cmd,
					alloc_buffer.indirect_draw_args.clone().reinterpret(),
					|cmd| {
						cmd.bind_descriptor_sets(
							PipelineBindPoint::Graphics,
							self.pipeline.layout().clone(),
							1,
							alloc_buffer.descriptor_set.clone(),
						)?;
						frame_context.modify()(cmd)
					},
					Params {
						frame_data: frame_context.frame_data_desc,
						scene: scene.scene.to_transient(frame_context.fif),
						sampler: self.sampler.to_transient(frame_context.fif),
					},
				)
				.unwrap();
		}
	}
}
