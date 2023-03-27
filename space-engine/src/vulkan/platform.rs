use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use vulkano::instance::LayerProperties;
use vulkano::VulkanLibrary;

pub struct VulkanLayers {
	validation_layers: HashSet<LayerPropertiesWrapper>,
}

impl VulkanLayers {
	pub fn new(lib: &Arc<VulkanLibrary>) -> Self {
		VulkanLayers {
			validation_layers: lib.layer_properties().unwrap().map(LayerPropertiesWrapper::new).collect(),
		}
	}
}

impl Deref for VulkanLayers {
	type Target = HashSet<LayerPropertiesWrapper>;

	fn deref(&self) -> &Self::Target {
		&self.validation_layers
	}
}

pub struct LayerPropertiesWrapper {
	inner: LayerProperties,
}

impl LayerPropertiesWrapper {
	fn new(layer_properties: LayerProperties) -> LayerPropertiesWrapper {
		LayerPropertiesWrapper {
			inner: layer_properties,
		}
	}
}

impl Deref for LayerPropertiesWrapper {
	type Target = LayerProperties;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl Borrow<str> for LayerPropertiesWrapper {
	fn borrow(&self) -> &str {
		self.name()
	}
}

impl AsRef<LayerProperties> for LayerPropertiesWrapper {
	fn as_ref(&self) -> &LayerProperties {
		&self.inner
	}
}

impl PartialEq<Self> for LayerPropertiesWrapper {
	fn eq(&self, other: &Self) -> bool {
		self.name().deref().eq(other.name().deref())
	}
}

impl Eq for LayerPropertiesWrapper {}

impl Hash for LayerPropertiesWrapper {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name().hash(state)
	}
}
