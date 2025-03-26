use rust_gpu_bindless_shader_builder::spirv_builder::{Capability, ShaderPanicStrategy, SpirvMetadata};
use rust_gpu_bindless_shader_builder::ShaderSymbolsBuilder;

fn main() -> anyhow::Result<()> {
	ShaderSymbolsBuilder::new("integration-test-shader", "spirv-unknown-vulkan1.2")
		.capability(Capability::GroupNonUniform)
		.capability(Capability::GroupNonUniformBallot)
		.capability(Capability::StorageImageExtendedFormats)
		.capability(Capability::StorageImageReadWithoutFormat)
		.capability(Capability::StorageImageWriteWithoutFormat)
		.capability(Capability::ShaderNonUniform)
		.spirv_metadata(SpirvMetadata::Full)
		.shader_panic_strategy(ShaderPanicStrategy::DebugPrintfThenExit {
			print_inputs: true,
			print_backtrace: true,
		})
		.build()?;
	Ok(())
}
