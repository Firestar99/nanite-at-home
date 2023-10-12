use std::sync::Arc;

use vulkano::device::{DeviceExtensions, Features};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::InstanceExtensions;
use vulkano::swapchain::Surface;
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VulkanLayers;
use crate::vulkan::window::event_loop::EventLoopAccess;

pub struct WindowPlugin {
	window_extensions: InstanceExtensions,
}

impl WindowPlugin {
	pub async fn new(event_loop: EventLoopAccess) -> Self {
		Self {
			window_extensions: event_loop.spawn(|event_loop| Surface::required_extensions(event_loop)).await,
		}
	}
}

impl Plugin for WindowPlugin {
	fn instance_config(&mut self, _library: &Arc<VulkanLibrary>, _layers: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		(self.window_extensions, Vec::new())
	}

	fn device_config(&mut self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, Features) {
		(DeviceExtensions {
			khr_swapchain: true,
			..Default::default()
		}, Features::empty())
	}
}
