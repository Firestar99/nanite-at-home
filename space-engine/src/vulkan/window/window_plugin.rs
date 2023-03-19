use std::sync::Arc;

use vulkano::device::{DeviceExtensions, Features};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceExtensions};
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VulkanLayers;

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
	fn instance_config(&mut self, _library: &Arc<VulkanLibrary>, _layers: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		(vulkano_win::required_extensions(_library), Vec::new())
	}

	fn device_config(&mut self, _library: &Arc<VulkanLibrary>, _instance: &Arc<Instance>, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions {
			khr_swapchain: true,
			..Default::default()
		}, Features::empty())
	}
}
