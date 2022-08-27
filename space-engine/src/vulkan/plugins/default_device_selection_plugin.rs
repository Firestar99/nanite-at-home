use std::sync::Arc;

use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::instance::Instance;

use crate::vulkan::init::Plugin;

struct DefaultDeviceSelectionPlugin {}

impl Plugin for DefaultDeviceSelectionPlugin {
	fn physical_device_filter(&mut self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice) -> Option<i32> {
		Some(match _physical_device.properties().device_type {
			PhysicalDeviceType::DiscreteGpu => 4,
			PhysicalDeviceType::IntegratedGpu => 3,
			PhysicalDeviceType::VirtualGpu => 2,
			PhysicalDeviceType::Cpu => 1,
			PhysicalDeviceType::Other => 0,
		})
	}
}
