use rust_gpu_bindless_shader_builder::ShaderSymbolsBuilder;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
	ShaderSymbolsBuilder::new_relative_path(
		"egui-shaders",
		"rust_gpu_bindless_egui_shaders",
		"spirv-unknown-vulkan1.2",
	)
	.build()?;
	Ok(())
}
