use vulkano::shader::ShaderStages;

pub mod descriptor_set_creator;
pub mod global_descriptor_set;
pub mod lod_obj;
pub mod model;
pub mod render_graph;
pub mod renderer_plugin;
pub mod renderers;

pub const ALL_SHADER_STAGES: ShaderStages = ShaderStages::VERTEX
	.union(ShaderStages::FRAGMENT)
	.union(ShaderStages::MESH)
	.union(ShaderStages::COMPUTE);
