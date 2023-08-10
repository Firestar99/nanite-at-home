#![cfg(not(target_arch = "spirv"))]

use std::ops::Deref;
use std::sync::Arc;

use vulkano::device::Device;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::shader::ShaderModule;

use crate::triangle::triangle_model::{TriangleVertex};
use crate::triangle::triangle_renderpass::TriangleRenderpass;
use crate::triangle::triangle_renderpass::TriangleRenderpassSubpass::MAIN;

pub struct TrianglePipeline(Arc<GraphicsPipeline>);

impl TrianglePipeline {
	pub fn new(device: &Arc<Device>, render_pass: &TriangleRenderpass) -> Self {
		mod vs {
			vulkano_shaders::shader! {
				bytes: "../target/spirv-builder/spirv-unknown-spv1.3/release/deps/triangle_example.spvs/triangle-triangle_shader-bla_vs.spv",
				ty: "vertex",
        	}
		}

		mod fs {
			vulkano_shaders::shader! {
				bytes: "../target/spirv-builder/spirv-unknown-spv1.3/release/deps/triangle_example.spvs/triangle-triangle_shader-bla_fs.spv",
            	ty: "fragment",
			}
		}

		let vs: Arc<ShaderModule> = vs::load(device.clone()).unwrap();
		let vs = vs.entry_point("triangle::triangle_shader::bla_vs").unwrap();
		let fs: Arc<ShaderModule> = fs::load(device.clone()).unwrap();
		let fs = fs.entry_point("triangle::triangle_shader::bla_fs").unwrap();

		let pipeline = GraphicsPipeline::start()
			.render_pass(render_pass.subpass(MAIN))
			.vertex_input_state(TriangleVertex::per_vertex())
			.input_assembly_state(InputAssemblyState::new())
			.vertex_shader(vs, ())
			.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
			.fragment_shader(fs, ())
			.build(device.clone())
			.unwrap();

		Self(pipeline)
	}
}

impl Deref for TrianglePipeline {
	type Target = Arc<GraphicsPipeline>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
