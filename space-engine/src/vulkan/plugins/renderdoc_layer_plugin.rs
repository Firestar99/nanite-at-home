use std::sync::Arc;

use smallvec::{smallvec, SmallVec};
use vulkano::instance::InstanceExtensions;
use vulkano::VulkanLibrary;

use crate::vulkan::init::Plugin;
use crate::vulkan::validation_layers::ValidationLayers;

pub const RENDERDOC_LAYER_NAME: &str = "VK_LAYER_RENDERDOC_Capture";

pub struct RenderdocLayerPlugin;

impl Plugin for RenderdocLayerPlugin {
	fn instance_config(
		&self,
		_library: &Arc<VulkanLibrary>,
		_layers: &ValidationLayers,
	) -> (InstanceExtensions, SmallVec<[String; 1]>) {
		assert!(
			_layers.contains(RENDERDOC_LAYER_NAME),
			"RenderDoc Layer is not available!"
		);
		(
			InstanceExtensions::empty(),
			smallvec![String::from(RENDERDOC_LAYER_NAME)],
		)
	}
}
