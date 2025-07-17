use rust_gpu_bindless_shader_builder::{ShaderSymbolsBuilder, anyhow};

fn main() -> anyhow::Result<()> {
	ShaderSymbolsBuilder::new_relative_path(
		"egui-shaders",
		"rust_gpu_bindless_egui_shaders",
		"spirv-unknown-vulkan1.2",
	)?
	.target_dir_path("spirv-builder-egui-shaders")
	.build()?;
	Ok(())
}
