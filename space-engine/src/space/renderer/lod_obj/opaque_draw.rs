use smallvec::smallvec;
use space_engine_shader::space::renderer::lod_obj::opaque_shader::Params;
use space_engine_shader::space::renderer::model::gpu_model::OpaqueGpuModel;
use std::mem;
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
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::{PipelineLayoutCreateInfo, PushConstantRange};
use vulkano::pipeline::{
	DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};
use vulkano::shader::ShaderStages;
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::descriptor::{Buffer, Sampler};

use crate::shader::space::renderer::lod_obj::opaque_shader;
use crate::space::renderer::global_descriptor_set::GlobalDescriptorSetLayout;
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: Arc<GraphicsPipeline>,
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
				push_constant_ranges: [PushConstantRange {
					stages: ShaderStages::TASK | ShaderStages::MESH | ShaderStages::FRAGMENT,
					offset: 0,
					size: mem::size_of::<Params>() as u32,
				}]
				.to_vec(),
				..PipelineLayoutCreateInfo::default()
			},
		)
		.unwrap();

		let pipeline = GraphicsPipeline::new(
			device.clone(),
			Some(init.pipeline_cache.deref().clone()),
			GraphicsPipelineCreateInfo {
				stages: smallvec![
					PipelineShaderStageCreateInfo::new(opaque_shader::opaque_task::new(device.clone())),
					PipelineShaderStageCreateInfo::new(opaque_shader::opaque_mesh::new(device.clone())),
					PipelineShaderStageCreateInfo::new(opaque_shader::opaque_fs::new(device.clone())),
				],
				rasterization_state: Some(RasterizationState::default()),
				viewport_state: Some(ViewportState::default()),
				multisample_state: Some(MultisampleState::default()),
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
				dynamic_state: [DynamicState::Viewport].into_iter().collect(),
				..GraphicsPipelineCreateInfo::layout(layout)
			},
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
			let init = &frame_context.render_context.init;
			cmd.bind_pipeline_graphics(self.pipeline.clone())
				.unwrap()
				.set_viewport(0, frame_context.viewport_smallvec())
				.unwrap()
				.bind_descriptor_sets(
					PipelineBindPoint::Graphics,
					self.pipeline.layout().clone(),
					0,
					(
						init.bindless.descriptor_set.clone(),
						frame_context.global_descriptor_set.clone().0,
					),
				)
				.unwrap()
				.push_constants(
					self.pipeline.layout().clone(),
					0,
					Params {
						models: models.to_transient(frame_context.fif),
						sampler: self.sampler.to_transient(frame_context.fif),
					}
					.to_static(),
				)
				.unwrap()
				.draw_mesh_tasks([models.len() as u32, 1, 1])
				.unwrap();
		}
	}
}
