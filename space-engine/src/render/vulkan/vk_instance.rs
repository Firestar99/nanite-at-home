use std::collections::HashMap;
use std::sync::Arc;

use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions, layers_list};
use vulkano::Version;

use crate::application_config::ApplicationConfig;

pub static ENGINE_NAME: &'static str = "space-rust";
pub static ENGINE_VERSION: Version = Version {
    major: 1,
    minor: 0,
    patch: 0,
};

pub struct VkInstanceConfig {
    pub application: ApplicationConfig,
    /// request layers to be enabled if available
    pub requested_layers: Vec<String>,
    /// request extensions to be enabled if available
    pub requested_extensions: InstanceExtensions,
}

impl VkInstanceConfig {
    pub fn new(application: ApplicationConfig) -> VkInstanceConfig {
        VkInstanceConfig {
            application: application,
            requested_layers: Vec::new(),
            requested_extensions: InstanceExtensions::none(),
        }
    }
}

pub struct VkInstanceService {
    instance: Arc<Instance>,
}

impl VkInstanceService {
    pub fn new(config: VkInstanceConfig) -> VkInstanceService {
        let mut requested_layers: HashMap<String, bool> = HashMap::from_iter(
            config.requested_layers.into_iter()
                .map(|x| {
                    (x, false)
                })
        );
        for layer in layers_list().unwrap() {
            if let Some(entry) = requested_layers.get_mut(&String::from(layer.name())) {
                *entry = true;
            }
        }

        let extensions = InstanceExtensions::supported_by_core().unwrap()
            .intersection(&config.requested_extensions);

        VkInstanceService {
            instance: Instance::new(InstanceCreateInfo {
                engine_name: Some(String::from(ENGINE_NAME)),
                engine_version: ENGINE_VERSION,
                application_name: Some(String::from(config.application.name)),
                application_version: Version::from(config.application.version),
                enabled_extensions: extensions,
                enabled_layers: Vec::from_iter(requested_layers.into_iter()
                    .filter(|x| x.1)
                    .map(|x| x.0)),
                ..Default::default()
            }).unwrap()
        }
    }

    pub fn instance(&self) -> &Instance {
        &*self.instance
    }
}