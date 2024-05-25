use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::metadata::Metadata;

pub use vulkano_bindless_shaders::desc_buffer::*;

pub struct MetadataCpu {
	metadata: Metadata,
}

impl MetadataCpu {
	pub fn new() -> Self {
		Self { metadata: Metadata }
	}
}

impl Deref for MetadataCpu {
	type Target = Metadata;

	fn deref(&self) -> &Self::Target {
		&self.metadata
	}
}

impl MetadataCpuInterface for MetadataCpu {}
