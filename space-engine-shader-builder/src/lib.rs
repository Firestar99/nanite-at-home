use crate::spirv_builder::ShaderPanicStrategy;
pub use shader_symbols_builder;
pub use shader_symbols_builder::spirv_builder;
use shader_symbols_builder::spirv_builder::{Capability, SpirvMetadata};
use shader_symbols_builder::ShaderSymbolsBuilder;

pub fn shader_symbols_builder_configured_for_space_engine(shader_crate: &str) -> ShaderSymbolsBuilder {
	ShaderSymbolsBuilder::new(shader_crate, "spirv-unknown-vulkan1.2")
		.capability(Capability::RuntimeDescriptorArray)
		.capability(Capability::MeshShadingEXT)
		.extension("SPV_EXT_mesh_shader")
		.with_spirv_builder(|b| b.spirv_metadata(SpirvMetadata::Full))
		.with_spirv_builder(|b| {
			b.shader_panic_strategy(ShaderPanicStrategy::DebugPrintfThenExit {
				print_inputs: true,
				print_backtrace: true,
			})
		})
}
