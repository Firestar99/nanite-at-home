use std::error::Error;

use space_engine_shader_builder::shader_symbols_builder_configured_for_space_engine;

fn main() -> Result<(), Box<dyn Error>> {
	shader_symbols_builder_configured_for_space_engine("rust-gpu-bindless-egui-shaders").build()?;
	Ok(())
}
