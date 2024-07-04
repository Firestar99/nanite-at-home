use space_asset_pipeline::meshlet::build_script::build;
use space_asset_pipeline::meshlet::codegen::codegen;
use std::error::Error;
use std::path::Path;
use std::{env, fs};

const MODELS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../models");

fn main() -> Result<(), Box<dyn Error>> {
	#[cfg(feature = "profile-with-puffin")]
	let _puffin_server = {
		profiling::puffin::set_scopes_on(true);
		let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
		puffin_http::Server::new(&server_addr).unwrap()
	};

	let result = inner_main();
	profiling::finish_frame!();
	result
}

fn inner_main() -> Result<(), Box<dyn Error>> {
	let out_dir = env::var("OUT_DIR").unwrap();
	let out_dir = Path::new(&out_dir);
	let models = build(Path::new(MODELS_DIR), out_dir, true)?;
	fs::write(out_dir.join("models.rs"), codegen(models.iter())?.to_string())?;
	Ok(())
}
