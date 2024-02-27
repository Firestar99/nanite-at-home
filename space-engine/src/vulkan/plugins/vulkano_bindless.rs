use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, Features};

use crate::vulkan::init::Plugin;

pub struct VulkanoBindless;

impl Plugin for VulkanoBindless {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(
			DeviceExtensions::empty(),
			vulkano_bindless::required_features::REQUIRED_FEATURES,
		)
	}
}
