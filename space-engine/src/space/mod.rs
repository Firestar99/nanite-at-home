pub mod cli_args;
pub mod queue_allocation;
pub mod renderer;

pub type Init = crate::vulkan::init::Init<queue_allocation::Queues>;
