use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use vulkano::{instance, VulkanLibrary};

pub struct ValidationLayers(HashSet<LayerProperties>);

impl ValidationLayers {
	pub fn new(lib: &Arc<VulkanLibrary>) -> Self {
		ValidationLayers(lib.layer_properties().unwrap().map(LayerProperties).collect())
	}
}

impl Deref for ValidationLayers {
	type Target = HashSet<LayerProperties>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Debug for ValidationLayers {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

#[derive(Clone)]
pub struct LayerProperties(instance::LayerProperties);

impl Deref for LayerProperties {
	type Target = instance::LayerProperties;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl AsRef<<LayerProperties as Deref>::Target> for LayerProperties {
	fn as_ref(&self) -> &<LayerProperties as Deref>::Target {
		self
	}
}

impl Debug for LayerProperties {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LayerProperties")
			.field("name", &self.name())
			.field("implementation_version", &self.implementation_version())
			.field("vulkan_version", &self.vulkan_version())
			.finish()
	}
}

// HashSet requirements
impl Borrow<str> for LayerProperties {
	fn borrow(&self) -> &str {
		self.name()
	}
}

impl Hash for LayerProperties {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name().hash(state)
	}
}

impl PartialEq<Self> for LayerProperties {
	fn eq(&self, other: &Self) -> bool {
		self.name().eq(other.name())
	}
}

impl Eq for LayerProperties {}
