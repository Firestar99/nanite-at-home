#![cfg(not(target_arch = "spirv"))]

use std::ops::Deref;
use std::sync::Arc;

use smallvec::{smallvec, SmallVec};
use vulkano::device::Device;
use vulkano::pipeline::{GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;

use crate::triangle::triangle_model::TriangleVertex;
use crate::triangle::triangle_renderpass::TriangleRenderpass;
use crate::triangle::triangle_renderpass::TriangleRenderpassSubpass::MAIN;

pub struct TrianglePipeline(Arc<GraphicsPipeline>);

impl TrianglePipeline {
	pub fn new(device: &Arc<Device>, render_pass: &TriangleRenderpass) -> Self {
		mod vs {
			vulkano_shaders::shader! {
				ty: "vertex",
				root_path_env: "SHADER_OUT_DIR",
				bytes: "triangle-triangle_shader-bla_vs.spv",
        	}
		}

		mod fs {
			vulkano_shaders::shader! {
            	ty: "fragment",
				root_path_env: "SHADER_OUT_DIR",
				bytes: "triangle-triangle_shader-bla_fs.spv",
			}
		}

		let stages: SmallVec<[PipelineShaderStageCreateInfo; 5]> = smallvec![
			PipelineShaderStageCreateInfo::new(vs::load(device.clone()).unwrap().entry_point("triangle::triangle_shader::bla_vs").unwrap()),
			PipelineShaderStageCreateInfo::new(fs::load(device.clone()).unwrap().entry_point("triangle::triangle_shader::bla_fs").unwrap()),
		];

		let vertex_input_state = TriangleVertex::per_vertex()
			.definition(&stages.first().unwrap().entry_point.info().input_interface)
			.unwrap();
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
				.into_pipeline_layout_create_info(device.clone())
				.unwrap(),
		).unwrap();

		let pipeline = GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo {
			stages,
			subpass: Some(render_pass.subpass(MAIN).into()),
			vertex_input_state: vertex_input_state.into(),
			input_assembly_state: InputAssemblyState::default().into(),
			rasterization_state: RasterizationState::default().into(),
			viewport_state: ViewportState::viewport_dynamic_scissor_irrelevant().into(),
			multisample_state: MultisampleState::default().into(),
			color_blend_state: ColorBlendState::default().into(),
			..GraphicsPipelineCreateInfo::layout(layout)
		}).unwrap();

		Self(pipeline)
	}
}

impl Deref for TrianglePipeline {
	type Target = Arc<GraphicsPipeline>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
