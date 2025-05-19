use rust_gpu_bindless_shader_builder::ShaderSymbolsBuilder;

fn main() -> anyhow::Result<()> {
	ShaderSymbolsBuilder::new("integration-test-shader", "spirv-unknown-vulkan1.2")?.build()?;
	Ok(())
}
