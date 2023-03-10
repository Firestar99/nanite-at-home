use space_engine::application_config::ApplicationConfig;
use space_engine::generate_application_config;

pub mod cli_args;
pub mod vulkan;
pub mod bootup;

pub const APPLICATION_CONFIG: ApplicationConfig = generate_application_config!();

