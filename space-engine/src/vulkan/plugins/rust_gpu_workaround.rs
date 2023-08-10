use std::sync::Arc;

use vulkano::device::{DeviceExtensions, Features};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::Instance;
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;

pub struct RustGpuWorkaround;

impl Plugin for RustGpuWorkaround {
	fn device_config(&mut self, _library: &Arc<VulkanLibrary>, _instance: &Arc<Instance>, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions::empty(), Features {
			vulkan_memory_model: true,
			..Features::default()
		})
	}
}
