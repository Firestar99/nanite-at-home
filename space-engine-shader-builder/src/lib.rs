use rust_gpu_bindless_shader_builder::spirv_builder::{Capability, ShaderPanicStrategy, SpirvMetadata};
use rust_gpu_bindless_shader_builder::{ShaderSymbolsBuilder, anyhow};

pub use rust_gpu_bindless_shader_builder;

pub fn shader_symbols_builder_configured_for_space_engine(shader_crate: &str) -> anyhow::Result<ShaderSymbolsBuilder> {
	Ok(ShaderSymbolsBuilder::new(shader_crate, "spirv-unknown-vulkan1.2")?
		.capability(Capability::MeshShadingEXT)
		.capability(Capability::GroupNonUniform)
		.capability(Capability::GroupNonUniformBallot)
		.capability(Capability::StorageImageExtendedFormats)
		.capability(Capability::StorageImageReadWithoutFormat)
		.capability(Capability::StorageImageWriteWithoutFormat)
		.capability(Capability::ShaderNonUniform)
		.extension("SPV_EXT_mesh_shader")
		.spirv_metadata(SpirvMetadata::Full)
		.shader_panic_strategy(ShaderPanicStrategy::DebugPrintfThenExit {
			print_inputs: true,
			print_backtrace: true,
		}))
}
