use crate::application_config::ApplicationConfig;
use crate::generate_application_config;

pub mod platform;
pub mod init;
pub mod plugins;

pub const ENGINE_APPLICATION_CONFIG: ApplicationConfig = generate_application_config!();
