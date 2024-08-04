pub mod lighting;
pub mod meshlet;
pub mod queue_allocation;
pub mod render_graph;
pub mod renderer_plugin;
pub mod renderers;

pub type Init = crate::device::init::Init<queue_allocation::Queues>;
