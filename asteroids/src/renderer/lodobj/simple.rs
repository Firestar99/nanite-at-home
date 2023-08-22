use std::ops::Deref;
use std::sync::Arc;

use smallvec::smallvec;
use vulkano::device::Device;
use vulkano::pipeline::{GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;

use crate::renderer::renderpass3d::RenderPass3D;
use crate::shader::renderer::lodobj::{bla_fs, bla_vs};

pub struct SimplePipeline(Arc<GraphicsPipeline>);

impl SimplePipeline {
	pub fn new(device: &Arc<Device>, render_pass: &RenderPass3D) -> Self {
		let stages = smallvec![
			PipelineShaderStageCreateInfo::new(bla_vs::new(device.clone())),
			PipelineShaderStageCreateInfo::new(bla_fs::new(device.clone())),
		];
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
				.into_pipeline_layout_create_info(device.clone())
				.unwrap(),
		).unwrap();

		// may need more things specified? Or are the defaults plenty?
		Self(GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo {
			stages,
			subpass: Some(render_pass.subpass_main().into()),
			vertex_input_state: Some(VertexInputState::default()),
			input_assembly_state: Some(InputAssemblyState::default()),
			..GraphicsPipelineCreateInfo::layout(layout)
		}).unwrap())
	}
}

impl Deref for SimplePipeline {
	type Target = Arc<GraphicsPipeline>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
