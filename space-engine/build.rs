use std::error::Error;

use shader_symbols_builder::ShaderSymbolsBuilder;

fn main() -> Result<(), Box<dyn Error>> {
	ShaderSymbolsBuilder::new("space-engine-shader", "spirv-unknown-vulkan1.2")
		.build()?;
	Ok(())
}
