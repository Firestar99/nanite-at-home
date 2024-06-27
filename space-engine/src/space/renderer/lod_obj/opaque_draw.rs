use space_engine_shader::space::renderer::lod_obj::opaque_model::OpaqueModel;
use space_engine_shader::space::renderer::lod_obj::opaque_shader::Params;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::format::Format;
use vulkano::image::sampler::SamplerCreateInfo;
use vulkano::pipeline::graphics::color_blend::{
	AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::DynamicState;
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::descriptor::{Buffer, RCDescExt, Sampler};
use vulkano_bindless::pipeline::mesh_graphics_pipeline::{
	BindlessMeshGraphicsPipeline, MeshGraphicsPipelineCreateInfo,
};

use crate::shader::space::renderer::lod_obj::opaque_shader;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: BindlessMeshGraphicsPipeline<Params<'static>>,
	sampler: RCDesc<Sampler>,
}

impl OpaqueDrawPipeline {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let pipeline = BindlessMeshGraphicsPipeline::new_task(
			init.bindless.clone(),
			opaque_shader::opaque_task::new(),
			opaque_shader::opaque_mesh::new(),
			opaque_shader::opaque_fs::new(),
			MeshGraphicsPipelineCreateInfo {
				rasterization_state: RasterizationState::default(),
				viewport_state: ViewportState::default(),
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

		let sampler = init
			.bindless
			.sampler()
			.alloc(SamplerCreateInfo::simple_repeat_linear())
			.unwrap();

		Self { pipeline, sampler }
	}

	pub fn draw(
		&self,
		frame_context: &FrameContext,
		cmd: &mut RecordingCommandBuffer,
		models: RCDesc<Buffer<[OpaqueModel]>>,
	) {
		unsafe {
			self.pipeline
				.draw_mesh_tasks(
					cmd,
					[models.len() as u32, 1, 1],
					frame_context.modify(),
					Params {
						frame_data: frame_context.frame_data_desc,
						models: models.to_transient(frame_context.fif),
						sampler: self.sampler.to_transient(frame_context.fif),
					},
				)
				.unwrap();
		}
	}
}
