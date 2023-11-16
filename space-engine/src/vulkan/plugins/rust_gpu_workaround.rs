use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, Features};

use crate::vulkan::init::Plugin;

pub struct RustGpuWorkaround;

impl Plugin for RustGpuWorkaround {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(
			DeviceExtensions::empty(),
			Features {
				vulkan_memory_model: true,
				..Features::default()
			},
		)
	}
}
