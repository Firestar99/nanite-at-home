use space_engine_shader::space::renderer::lod_obj::opaque_shader::Params;
use space_engine_shader::space::renderer::model::gpu_model::OpaqueGpuModel;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::format::Format;
use vulkano::image::sampler::SamplerCreateInfo;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::PipelineBindPoint::Graphics;
use vulkano::pipeline::{DynamicState, Pipeline, PipelineLayout};
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::descriptor::{Buffer, Sampler};
use vulkano_bindless::pipeline::mesh_graphics_pipeline::{
	BindlessMeshGraphicsPipeline, MeshGraphicsPipelineCreateInfo,
};

use crate::shader::space::renderer::lod_obj::opaque_shader;
use crate::space::renderer::global_descriptor_set::GlobalDescriptorSetLayout;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: BindlessMeshGraphicsPipeline<Params<'static>>,
	sampler: RCDesc<Sampler>,
}

impl OpaqueDrawPipeline {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let device = &init.device;
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineLayoutCreateInfo {
				set_layouts: [
					init.bindless.descriptor_set_layout.clone(),
					GlobalDescriptorSetLayout::new(init).0,
				]
				.to_vec(),
				push_constant_ranges: init.bindless.get_push_constant::<Params<'static>>(),
				..PipelineLayoutCreateInfo::default()
			},
		)
		.unwrap();

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
						blend: Some(AttachmentBlend::alpha()),
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
			Some(layout),
		)
		.unwrap();

		let sampler = init
			.bindless
			.sampler
			.alloc(SamplerCreateInfo::simple_repeat_linear())
			.unwrap();

		Self { pipeline, sampler }
	}

	pub fn draw(
		&self,
		frame_context: &FrameContext,
		cmd: &mut RecordingCommandBuffer,
		models: RCDesc<Buffer<[OpaqueGpuModel]>>,
	) {
		unsafe {
			self.pipeline
				.draw_mesh_tasks(
					cmd,
					[models.len() as u32, 1, 1],
					|cmd| {
						cmd.bind_descriptor_sets(
							Graphics,
							self.pipeline.layout().clone(),
							1,
							frame_context.global_descriptor_set.clone().0,
						)?;
						frame_context.modify()(cmd)
					},
					Params {
						models: models.to_transient(frame_context.fif),
						sampler: self.sampler.to_transient(frame_context.fif),
					},
				)
				.unwrap();
		}
	}
}
