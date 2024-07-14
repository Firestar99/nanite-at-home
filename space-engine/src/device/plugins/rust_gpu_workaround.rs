use crate::device::init::Plugin;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};

pub struct RustGpuWorkaround;

impl Plugin for RustGpuWorkaround {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, DeviceFeatures) {
		(
			DeviceExtensions {
				khr_shader_non_semantic_info: true,
				..DeviceExtensions::empty()
			},
			DeviceFeatures {
				vulkan_memory_model: true,
				..DeviceFeatures::default()
			},
		)
	}
}
