use std::error::Error;

use shader_symbols_builder::ShaderSymbolsBuilder;

fn main() -> Result<(), Box<dyn Error>> {
	ShaderSymbolsBuilder::new("asteroids-shader", "spirv-unknown-vulkan1.2").build()?;
	Ok(())
}
