use std::ops::Deref;
use std::sync::Arc;

use smallvec::smallvec;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::{
	DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};

use crate::shader::space::renderer::lod_obj::opaque_shader::{opaque_fs, opaque_vs};
use crate::space::renderer::global_descriptor_set::GlobalDescriptorSetLayout;
use crate::space::renderer::model::model::OpaqueModel;
use crate::space::renderer::model::model_descriptor_set::ModelDescriptorSetLayout;
use crate::space::renderer::model::texture_array_descriptor_set::{
	TextureArrayDescriptorSet, TextureArrayDescriptorSetLayout,
};
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: Arc<GraphicsPipeline>,
}

impl OpaqueDrawPipeline {
	pub fn new(init: &Arc<Init>, format_color: Format, format_depth: Format) -> Self {
		let device = &init.device;
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineLayoutCreateInfo {
				set_layouts: [
					GlobalDescriptorSetLayout::new(init).0,
					ModelDescriptorSetLayout::new(init).0,
					TextureArrayDescriptorSetLayout::new(init).0,
				]
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
					PipelineShaderStageCreateInfo::new(opaque_vs::new(device.clone())),
					PipelineShaderStageCreateInfo::new(opaque_fs::new(device.clone())),
				],
				vertex_input_state: Some(VertexInputState::default()),
				input_assembly_state: Some(InputAssemblyState::default()),
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

		Self { pipeline }
	}

	pub fn draw(
		&self,
		frame_context: &FrameContext,
		cmd: &mut RecordingCommandBuffer,
		model: &OpaqueModel,
		texture_array_descriptor_set: &TextureArrayDescriptorSet,
	) {
		cmd.bind_pipeline_graphics(self.pipeline.clone())
			.unwrap()
			.set_viewport(0, frame_context.viewport_smallvec())
			.unwrap()
			.bind_descriptor_sets(
				PipelineBindPoint::Graphics,
				self.pipeline.layout().clone(),
				0,
				(
					frame_context.global_descriptor_set.clone().0,
					model.descriptor.clone().0,
					texture_array_descriptor_set.clone().0,
				),
			)
			.unwrap();
		unsafe {
			cmd.draw(model.vertex_buffer.len() as u32, 1, 0, 0).unwrap();
		}
	}

	pub fn descriptor_set_layout_model(&self) -> &Arc<DescriptorSetLayout> {
		&self.pipeline.layout().set_layouts()[1]
	}
}
