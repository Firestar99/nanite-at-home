use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use space_asset::meshlet::mesh2instance::{MeshletMesh2Instance, MeshletMesh2InstanceCpu};
use space_engine_shader::renderer::meshlet::mesh_shader::{Params, TASK_WG_SIZE};
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
use vulkano::pipeline::DynamicState;
use vulkano_bindless::descriptor::sampler::Sampler;
use vulkano_bindless::descriptor::{RCDesc, RCDescExt};
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
		g_albedo_format_srgb: Format,
		g_normal_format: Format,
		g_rm_format: Format,
		depth_format: Format,
	) -> Self {
		let pipeline = BindlessMeshGraphicsPipeline::new_task(
			init.bindless.clone(),
			crate::shader::renderer::meshlet::mesh_shader::meshlet_task::new(),
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
						Some(g_rm_format),
					],
					depth_attachment_format: Some(depth_format),
					..PipelineRenderingCreateInfo::default()
				})
				.into(),
				discard_rectangle_state: None,
				dynamic_state: [DynamicState::Viewport].into_iter().collect(),
				conservative_rasterization_state: None,
			},
			Some(init.pipeline_cache.deref().clone()),
			None,
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
		mesh2instance: &MeshletMesh2InstanceCpu,
	) {
		unsafe {
			let groups_x = (mesh2instance.num_meshlets + TASK_WG_SIZE - 1) / TASK_WG_SIZE;
			self.pipeline
				.draw_mesh_tasks(
					cmd,
					[groups_x, mesh2instance.instances.len() as u32, 1],
					frame_context.modify(),
					Params {
						frame_data: frame_context.frame_data_desc,
						mesh2instance: MeshletMesh2Instance {
							mesh: mesh2instance.mesh.to_transient(frame_context.fif),
							instances: mesh2instance.instances.to_transient(frame_context.fif),
						},
						sampler: self.sampler.to_transient(frame_context.fif),
					},
				)
				.unwrap();
		}
	}
}
