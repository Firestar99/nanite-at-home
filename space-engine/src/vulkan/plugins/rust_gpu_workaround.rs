use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};

use crate::vulkan::init::Plugin;

pub struct RustGpuWorkaround;

impl Plugin for RustGpuWorkaround {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, DeviceFeatures) {
		(
			DeviceExtensions::empty(),
			DeviceFeatures {
				vulkan_memory_model: true,
				..DeviceFeatures::default()
			},
		)
	}
}
