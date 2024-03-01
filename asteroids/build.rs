use space_engine_shader_builder::shader_symbols_builder_configured_for_space_engine;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
	shader_symbols_builder_configured_for_space_engine("asteroids-shader").build()?;
	Ok(())
}
