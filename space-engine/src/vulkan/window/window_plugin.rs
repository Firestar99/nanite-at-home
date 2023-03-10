use std::sync::Arc;

use vulkano::instance::InstanceExtensions;
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VulkanLayers;

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
	fn instance_config(&mut self, _library: &Arc<VulkanLibrary>, _layers: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		(vulkano_win::required_extensions(_library), Vec::new())
	}
}
