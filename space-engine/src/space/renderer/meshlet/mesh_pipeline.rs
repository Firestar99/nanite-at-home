use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;
use space_asset::meshlet::mesh::MeshletMesh2InstanceCpu;
use space_asset::meshlet::mesh2instance::MeshletMesh2Instance;
use space_engine_shader::space::renderer::meshlet::mesh_shader::{Params, TASK_WG_SIZE};
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{
	AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::DynamicState;
use vulkano_bindless::descriptor::RCDescExt;
use vulkano_bindless::pipeline::mesh_graphics_pipeline::{
	BindlessMeshGraphicsPipeline, MeshGraphicsPipelineCreateInfo,
};

pub struct MeshDrawPipeline {
	pipeline: BindlessMeshGraphicsPipeline<Params<'static>>,
}

impl MeshDrawPipeline {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline = BindlessMeshGraphicsPipeline::new_task(
			init.bindless.clone(),
			crate::shader::space::renderer::meshlet::mesh_shader::meshlet_task::new(),
			crate::shader::space::renderer::meshlet::mesh_shader::meshlet_mesh::new(),
			crate::shader::space::renderer::meshlet::mesh_shader::meshlet_frag_meshlet_id::new(),
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
					attachments: vec![ColorBlendAttachmentState {
						blend: Some(AttachmentBlend {
							src_color_blend_factor: BlendFactor::One,
							dst_color_blend_factor: BlendFactor::Zero,
							color_blend_op: BlendOp::Add,
							src_alpha_blend_factor: BlendFactor::One,
							dst_alpha_blend_factor: BlendFactor::Zero,
							alpha_blend_op: BlendOp::Add,
						}),
						..ColorBlendAttachmentState::default()
					}],
					..Default::default()
				}),
				subpass: PipelineSubpassType::BeginRendering(PipelineRenderingCreateInfo {
					color_attachment_formats: vec![Some(format_color)],
					depth_attachment_format: Some(format_depth),
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

		Self { pipeline }
	}

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
					},
				)
				.unwrap();
		}
	}
}
