use space_asset_pipeline::meshlet::build_script::build_script;
use std::env;
use std::error::Error;
use std::path::Path;

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
	build_script(Path::new(MODELS_DIR), out_dir, &out_dir.join("models.rs"), true)?;
	Ok(())
}
