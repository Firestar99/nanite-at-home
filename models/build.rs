use anyhow::Context;
use space_asset_preprocess::meshlet::build_script::{build_script, out_and_export_dir};
use std::env;
use std::path::Path;

const MODELS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/models");

#[allow(clippy::let_and_return)]
fn main() -> anyhow::Result<()> {
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

fn inner_main() -> anyhow::Result<()> {
	let (out_dir, export_dir) = out_and_export_dir().context("Failed to find export path")?;
	build_script(Path::new(MODELS_DIR), &export_dir, &out_dir.join("models.rs"), true)?;
	Ok(())
}
