use vulkano::device::DeviceExtensions;
use vulkano::instance::InstanceExtensions;

pub struct Plugin {
    pub optional: bool,
    pub instance_extensions: RequiredOrOptional<InstanceExtensions>,
    pub device_extensions: RequiredOrOptional<DeviceExtensions>,
}

pub struct RequiredOrOptional<T> {
    pub required: T,
    pub optional: T,
}

//workaround for InstanceExtensions and DeviceExtensions not impl Default
impl Default for RequiredOrOptional<InstanceExtensions> {
    fn default() -> Self {
        RequiredOrOptional {
            required: InstanceExtensions::none(),
            optional: InstanceExtensions::none(),
        }
    }
}

impl Default for RequiredOrOptional<DeviceExtensions> {
    fn default() -> Self {
        RequiredOrOptional {
            required: DeviceExtensions::none(),
            optional: DeviceExtensions::none(),
        }
    }
}
