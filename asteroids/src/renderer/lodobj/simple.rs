use std::ops::Deref;
use std::sync::Arc;

use vulkano::device::Device;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;

use crate::renderer::renderpass3d::RenderPass3D;
use crate::renderer::zstvertex::ZSTVertex;
use crate::shader::renderer::lodobj::{bla_fs, bla_vs};

pub struct SimplePipeline(Arc<GraphicsPipeline>);

impl SimplePipeline {
	pub fn new(device: &Arc<Device>, render_pass: &RenderPass3D) -> Self {
		Self(GraphicsPipeline::start()
			.render_pass(render_pass.subpass_main())
			.vertex_input_state(ZSTVertex::per_vertex())
			.input_assembly_state(InputAssemblyState::new())
			.vertex_shader(bla_vs::new(device.clone()).entry(), ())
			.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
			.fragment_shader(bla_fs::new(device.clone()).entry(), ())
			.build(device.clone())
			.unwrap())
	}
}

impl Deref for SimplePipeline {
	type Target = Arc<GraphicsPipeline>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
