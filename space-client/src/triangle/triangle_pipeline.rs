use std::ops::Deref;
use std::sync::Arc;

use vulkano::device::Device;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::shader::ShaderModule;

use crate::triangle::triangle_model::Vertex;
use crate::triangle::triangle_renderpass::TriangleRenderpass;
use crate::triangle::triangle_renderpass::TriangleRenderpassSubpass::MAIN;

pub struct TrianglePipeline(Arc<GraphicsPipeline>);

impl TrianglePipeline {
	pub fn new(device: &Arc<Device>, render_pass: &TriangleRenderpass) -> Self {
		mod vs {
			vulkano_shaders::shader! {
            ty: "vertex",
            src: "
				#version 450

				layout(location = 0) in vec2 position;

				void main() {
					gl_Position = vec4(position, 0.0, 1.0);
				}
			"
        }
		}

		mod fs {
			vulkano_shaders::shader! {
            ty: "fragment",
            src: "
				#version 450

				layout(location = 0) out vec4 f_color;

				void main() {
					f_color = vec4(1.0, 0.0, 0.0, 1.0);
				}
			"
        }
		}

		let vs: Arc<ShaderModule> = vs::load(device.clone()).unwrap();
		let vs = vs.entry_point("main").unwrap();
		let fs: Arc<ShaderModule> = fs::load(device.clone()).unwrap();
		let fs = fs.entry_point("main").unwrap();

		let pipeline = GraphicsPipeline::start()
			.render_pass(render_pass.subpass(MAIN))
			.vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
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
