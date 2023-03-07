use vulkano::instance::InstanceExtensions;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VulkanLayers;

pub const STANDARD_VALIDATION_LAYER_NAME: &str = "VK_LAYER_KHRONOS_validation";

pub struct StandardValidationLayerPlugin {}

impl Plugin for StandardValidationLayerPlugin {
	fn instance_config(&mut self, _platform: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		assert!(_platform.validation_layers.contains(STANDARD_VALIDATION_LAYER_NAME), "Standard Validation Layer is not available!");
		(InstanceExtensions::empty(), vec![STANDARD_VALIDATION_LAYER_NAME])
	}
}
