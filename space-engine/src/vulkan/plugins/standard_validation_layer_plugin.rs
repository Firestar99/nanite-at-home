use crate::vulkan::init::Plugin;
use crate::vulkan::validation_layers::ValidationLayers;
use smallvec::{smallvec, SmallVec};
use std::sync::Arc;
use vulkano::instance::InstanceExtensions;
use vulkano::VulkanLibrary;

pub const STANDARD_VALIDATION_LAYER_NAME: &str = "VK_LAYER_KHRONOS_validation";

pub struct StandardValidationLayerPlugin;

impl Plugin for StandardValidationLayerPlugin {
	fn instance_config(
		&self,
		_library: &Arc<VulkanLibrary>,
		_layers: &ValidationLayers,
	) -> (InstanceExtensions, SmallVec<[String; 1]>) {
		assert!(
			_layers.contains(STANDARD_VALIDATION_LAYER_NAME),
			"Standard Validation Layer is not available!"
		);
		(
			InstanceExtensions::empty(),
			smallvec![String::from(STANDARD_VALIDATION_LAYER_NAME)],
		)
	}
}
