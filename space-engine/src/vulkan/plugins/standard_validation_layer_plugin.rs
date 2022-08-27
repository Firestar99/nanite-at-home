use vulkano::instance::InstanceExtensions;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VkPlatform;

pub const STANDARD_VALIDATION_LAYER_NAME: &str = "VK_LAYER_KHRONOS_validation";

pub struct StandardValidationLayerPlugin {}

impl Plugin for StandardValidationLayerPlugin {
	fn instance_config(&mut self, _platform: &VkPlatform) -> (InstanceExtensions, Vec<&'static str>) {
		assert!(_platform.validation_layers.contains(STANDARD_VALIDATION_LAYER_NAME), "Standard Validation Layer is not available!");
		(InstanceExtensions::none(), vec![STANDARD_VALIDATION_LAYER_NAME])
	}
}
