use crate::reinit::{Reinit, Target};
use crate::space::engine_config::EngineConfig;

pub mod cli_args;
pub mod engine_config;
pub mod bootup;
pub mod queue_allocation;

pub fn init(engine_config: EngineConfig, target: &'static Reinit<impl Target>) {
	engine_config::init_config(engine_config);
	crate::vulkan::window::event_loop::event_loop_init(target);
}
