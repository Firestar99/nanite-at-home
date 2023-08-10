use std::env;
use std::error::Error;
use std::path::PathBuf;

use spirv_builder::{MetadataPrintout, SpirvBuilder, SpirvMetadata};

fn main() -> Result<(), Box<dyn Error>> {
	let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
	if target_arch == "spirv" {
		println!("exiting successfully to prevent recursion: target arch is spir-v");
		return Ok(());
	}

	let manifest_dir = env!("CARGO_MANIFEST_DIR");
	// let crate_path = [manifest_dir, "..", "triangle-example-shader"].iter().copied().collect::<PathBuf>();
	let crate_path = PathBuf::from(manifest_dir);
	let result = SpirvBuilder::new(crate_path, "spirv-unknown-spv1.3")
		.multimodule(true)
		.spirv_metadata(SpirvMetadata::NameVariables)
		.print_metadata(MetadataPrintout::None)
		.build()?;

	// // let module_path = result.module.unwrap_single();
	// // let data = std::fs::read(module_path).unwrap();

	let paths = result.module.unwrap_multi().iter()
		.map(|x| format!("{}: {}", x.0, x.1.to_str().unwrap()))
		.collect::<Vec<String>>().join("\n");
	println!("path: {}", paths);
	println!("entry points: {}", result.codegen_entry_point_strings());
	// Err(Box::new(SpirvBuilderError::BuildFailed))
	Ok(())
}
