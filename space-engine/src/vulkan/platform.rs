use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::Deref;
use std::pin::Pin;

use vulkano::instance::{InstanceExtensions, LayerProperties, layers_list};

pub struct VkPlatform {
	pub instance_extensions: InstanceExtensions,
	pub validation_layers: HashSet<LayerPropertiesWrapper>,
}

pub struct LayerPropertiesWrapper {
	name: Pin<&'static str>,
	layer_properties: LayerProperties,
}

impl LayerPropertiesWrapper {
	fn new(layer_properties: LayerProperties) -> LayerPropertiesWrapper {
		LayerPropertiesWrapper {
			name: Pin::new(unsafe {
				mem::transmute::<&str, &'static str>(layer_properties.name())
			}),
			layer_properties,
		}
	}
}

impl Borrow<str> for LayerPropertiesWrapper {
	fn borrow(&self) -> &str {
		&self.name
	}
}

impl AsRef<LayerProperties> for LayerPropertiesWrapper {
	fn as_ref(&self) -> &LayerProperties {
		&self.layer_properties
	}
}

impl PartialEq<Self> for LayerPropertiesWrapper {
	fn eq(&self, other: &Self) -> bool {
		self.name.deref().eq(other.name.deref())
	}
}

impl Eq for LayerPropertiesWrapper {}

impl Hash for LayerPropertiesWrapper {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name.hash(state)
	}
}

impl VkPlatform {
	pub fn new() -> Self {
		VkPlatform {
			instance_extensions: InstanceExtensions::supported_by_core().unwrap(),
			validation_layers: layers_list().unwrap().map(|l| LayerPropertiesWrapper::new(l)).collect(),
		}
	}
}
