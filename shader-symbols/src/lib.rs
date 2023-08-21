use std::sync::Arc;

use vulkano::shader::{EntryPoint, ShaderModule};

pub struct Shader {
	pub shader_module: Arc<ShaderModule>,
	pub entry_point: &'static str,
}

impl Shader {
	pub fn entry(&self) -> EntryPoint {
		self.shader_module.entry_point(self.entry_point).unwrap()
	}
}

#[macro_export]
macro_rules! include_shader {
	() => {
		include!(concat!(env!("OUT_DIR"), "/shader_symbols.rs"));
	}
}
