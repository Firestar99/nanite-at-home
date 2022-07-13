use vulkano::instance::InstanceExtensions;
use vulkano_win::required_extensions;

use crate::render::plugin::plugin::{Plugin, RequiredOrOptional};

pub struct WindowService {}

impl WindowService {
    pub fn plugin() -> Plugin {
        Plugin {
            optional: false,
            instance_extensions: RequiredOrOptional {
                required: required_extensions(),
                optional: InstanceExtensions::none(),
            },
            device_extensions: Default::default(),
        }
    }
}


