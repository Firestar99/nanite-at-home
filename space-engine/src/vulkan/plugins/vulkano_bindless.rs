use crate::vulkan::init::Plugin;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};

pub struct VulkanoBindless;

impl Plugin for VulkanoBindless {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, DeviceFeatures) {
		(
			DeviceExtensions::empty(),
			vulkano_bindless::required_features::REQUIRED_FEATURES,
		)
	}
}
