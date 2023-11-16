use std::sync::Arc;

use smallvec::smallvec;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{
	DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};

use crate::shader::space::renderer::lod_obj::opaque_shader::{opaque_fs, opaque_vs};
use crate::space::renderer::frame_in_flight::ResourceInFlight;
use crate::space::renderer::lod_obj::opaque_model::OpaqueModel;
use crate::space::renderer::render_graph::context::{FrameContext, RenderContext};

#[derive(Clone)]
pub struct OpaqueDrawPipeline {
	pipeline: Arc<GraphicsPipeline>,
	descriptor_set: ResourceInFlight<Arc<PersistentDescriptorSet>>,
}

impl OpaqueDrawPipeline {
	pub fn new(context: &Arc<RenderContext>, format_color: Format) -> Self {
		let device = &context.init.device;
		let stages = smallvec![
			PipelineShaderStageCreateInfo::new(opaque_vs::new(device.clone())),
			PipelineShaderStageCreateInfo::new(opaque_fs::new(device.clone())),
		];
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
				.into_pipeline_layout_create_info(device.clone())
				.unwrap(),
		)
		.unwrap();
		let descriptor_set = ResourceInFlight::new(context, |frame| {
			PersistentDescriptorSet::new(
				&context.init.descriptor_allocator,
				layout.set_layouts()[0].clone(),
				[WriteDescriptorSet::buffer(
					0,
					context.frame_data_uniform.index(frame).clone(),
				)],
				[],
			)
			.unwrap()
		});

		let pipeline = GraphicsPipeline::new(
			device.clone(),
			None,
			GraphicsPipelineCreateInfo {
				stages,
				vertex_input_state: VertexInputState::default().into(),
				input_assembly_state: InputAssemblyState::default().into(),
				rasterization_state: RasterizationState::default().into(),
				viewport_state: ViewportState::default().into(),
				multisample_state: MultisampleState::default().into(),
				color_blend_state: ColorBlendState {
					attachments: vec![ColorBlendAttachmentState::default()],
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

		Self {
			pipeline,
			descriptor_set,
		}
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
					self.descriptor_set.index(frame_context.frame_in_flight).clone(),
					model.descriptor.clone(),
				),
			)
			.unwrap()
			.draw(3, 1, 0, 0)
			.unwrap();
	}

	pub fn descriptor_set_layout_model(&self) -> &Arc<DescriptorSetLayout> {
		&self.pipeline.layout().set_layouts()[1]
	}
}
