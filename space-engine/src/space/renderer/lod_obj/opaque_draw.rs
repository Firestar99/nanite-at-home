use std::ops::Deref;
use std::sync::Arc;

use smallvec::smallvec;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState};
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
use crate::space::renderer::render_graph::context::FrameContext;
use crate::space::Init;

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: Arc<GraphicsPipeline>,
}

impl OpaqueDrawPipeline {
	pub fn new(init: &Arc<Init>, format_color: Format) -> Self {
		let device = &init.device;
		let stages = smallvec![
			PipelineShaderStageCreateInfo::new(opaque_vs::new(device.clone())),
			PipelineShaderStageCreateInfo::new(opaque_fs::new(device.clone())),
		];
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineLayoutCreateInfo {
				set_layouts: [
					GlobalDescriptorSetLayout::new(init).0,
					ModelDescriptorSetLayout::new(init).0,
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
				stages,
				vertex_input_state: VertexInputState::default().into(),
				input_assembly_state: InputAssemblyState::default().into(),
				rasterization_state: RasterizationState::default().into(),
				viewport_state: ViewportState::default().into(),
				multisample_state: MultisampleState::default().into(),
				color_blend_state: ColorBlendState {
					attachments: vec![ColorBlendAttachmentState {
						blend: AttachmentBlend::alpha().into(),
						..ColorBlendAttachmentState::default()
					}],
					..Default::default()
				}
				.into(),
				subpass: PipelineSubpassType::BeginRendering(PipelineRenderingCreateInfo {
					color_attachment_formats: vec![Some(format_color)],
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
		cmd: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
		model: &OpaqueModel,
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
				),
			)
			.unwrap()
			.draw(model.vertex_buffer.len() as u32, 1, 0, 0)
			.unwrap();
	}

	pub fn descriptor_set_layout_model(&self) -> &Arc<DescriptorSetLayout> {
		&self.pipeline.layout().set_layouts()[1]
	}
}
