use std::sync::Arc;

use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};

use crate::vulkan::init::Plugin;

pub struct RendererPlugin;

impl Plugin for RendererPlugin {
	fn device_config(&self, _physical_device: &Arc<PhysicalDevice>) -> (DeviceExtensions, DeviceFeatures) {
		(
			DeviceExtensions {
				ext_mesh_shader: true,
				..DeviceExtensions::default()
			},
			DeviceFeatures {
				dynamic_rendering: true,
				mesh_shader: true,
				..DeviceFeatures::default()
			},
		)
	}
}
