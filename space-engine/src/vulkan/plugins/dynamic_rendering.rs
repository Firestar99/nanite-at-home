use std::sync::Arc;

use vulkano::device::{DeviceExtensions, Features};
use vulkano::device::physical::PhysicalDevice;

use crate::vulkan::init::Plugin;

pub struct DynamicRendering;

impl Plugin for DynamicRendering {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions::default(), Features {
			dynamic_rendering: true,
			..Features::default()
		})
	}
}
