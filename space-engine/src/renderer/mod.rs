pub mod lod_obj;
pub mod meshlet;
pub mod model;
pub mod queue_allocation;
pub mod render_graph;
pub mod renderer_plugin;
pub mod renderers;

pub type Init = crate::device::init::Init<crate::renderer::queue_allocation::Queues>;
