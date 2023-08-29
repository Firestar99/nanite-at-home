use std::ops::Deref;
use std::sync::Arc;

use smallvec::smallvec;
use vulkano::device::Device;
use vulkano::pipeline::{GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::PipelineSubpassType;
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;

use crate::shader::renderer::lodobj::opaque::{opaque_fs, opaque_vs};
use crate::space::renderer::lodobj::render_task::rendering_info;

#[derive(Clone, Debug)]
pub struct OpaquePipeline(pub Arc<GraphicsPipeline>);

impl OpaquePipeline {
	pub fn new(device: &Arc<Device>) -> Self {
		let stages = smallvec![
			PipelineShaderStageCreateInfo::new(opaque_vs::new(device.clone())),
			PipelineShaderStageCreateInfo::new(opaque_fs::new(device.clone())),
		];
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
				.into_pipeline_layout_create_info(device.clone())
				.unwrap(),
		).unwrap();

		Self(GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo {
			stages,
			vertex_input_state: VertexInputState::default().into(),
			input_assembly_state: InputAssemblyState::default().into(),
			rasterization_state: RasterizationState::default().into(),
			viewport_state: ViewportState::viewport_dynamic_scissor_irrelevant().into(),
			multisample_state: MultisampleState::default().into(),
			color_blend_state: ColorBlendState::default().into(),
			subpass: PipelineSubpassType::BeginRendering(rendering_info().clone()).into(),
			..GraphicsPipelineCreateInfo::layout(layout)
		}).unwrap())
	}
}

impl Deref for OpaquePipeline {
	type Target = Arc<GraphicsPipeline>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
