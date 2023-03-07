use vulkano::instance::InstanceExtensions;

use crate::vulkan::init::Plugin;
use crate::vulkan::platform::VulkanLayers;

pub const RENDERDOC_LAYER_NAME: &str = "VK_LAYER_RENDERDOC_Capture";

pub struct RenderdocLayerPlugin {}

impl Plugin for RenderdocLayerPlugin {
	fn instance_config(&mut self, _platform: &VulkanLayers) -> (InstanceExtensions, Vec<&'static str>) {
		assert!(_platform.validation_layers.contains(RENDERDOC_LAYER_NAME), "RenderDoc Layer is not available!");
		(InstanceExtensions::empty(), vec![RENDERDOC_LAYER_NAME])
	}
}
