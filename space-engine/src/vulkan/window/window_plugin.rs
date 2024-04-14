use smallvec::SmallVec;
use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};
use vulkano::instance::InstanceExtensions;
use vulkano::swapchain::Surface;
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;
use crate::vulkan::validation_layers::ValidationLayers;
use crate::vulkan::window::event_loop::EventLoopExecutor;

pub struct WindowPlugin {
	window_extensions: InstanceExtensions,
}

impl WindowPlugin {
	pub async fn new(event_loop: &EventLoopExecutor) -> Self {
		Self {
			window_extensions: event_loop
				.spawn(|event_loop| Surface::required_extensions(event_loop).unwrap())
				.await,
		}
	}
}

impl Plugin for WindowPlugin {
	fn instance_config(
		&self,
		_library: &Arc<VulkanLibrary>,
		_layers: &ValidationLayers,
	) -> (InstanceExtensions, SmallVec<[String; 1]>) {
		(self.window_extensions, SmallVec::new())
	}

	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, DeviceFeatures) {
		(
			DeviceExtensions {
				khr_swapchain: true,
				..Default::default()
			},
			DeviceFeatures::empty(),
		)
	}
}
