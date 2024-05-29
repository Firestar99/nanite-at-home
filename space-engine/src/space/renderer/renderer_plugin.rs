use crate::vulkan::init::Plugin;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{DeviceExtensions, DeviceFeatures};

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
				task_shader: true,
				..DeviceFeatures::default()
			},
		)
	}
}
