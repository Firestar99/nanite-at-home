use quote::{format_ident, quote};
use rayon::prelude::*;
use space_asset_pipeline::meshlet::error::Error as MeshletError;
use space_asset_pipeline::meshlet::process::Gltf;
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{env, fs};

const LANTERN: &str = concat!(
	env!("CARGO_MANIFEST_DIR"),
	"/src/sample_scene/Lantern/glTF/Lantern.gltf"
);
const BISTRO: &str = "../../models/bistro/export/Bistro.gltf";
const BALL: &str = "../../models/glTF-Sample-Assets/Models/CarbonFibre/glTF/CarbonFibre.gltf";
const SPONZA: &str = "../../models/glTF-Sample-Assets/Models/Sponza/glTF/Sponza.gltf";

fn main() -> Result<(), Box<dyn Error>> {
	#[cfg(feature = "profile-with-puffin")]
	let _puffin_server = {
		profiling::puffin::set_scopes_on(true);
		let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
		puffin_http::Server::new(&server_addr).unwrap()
	};

	let result = build();
	profiling::finish_frame!();
	result
}

#[profiling::function]
fn build() -> Result<(), Box<dyn Error>> {
	let models = [LANTERN, BISTRO, BALL, SPONZA];

	let out_dir = env::var("OUT_DIR").unwrap();
	let out_dir = Path::new(&out_dir);
	let model_paths = models.map(|path| {
		// Rerun build script if dir containing gltf has changed. That is technically not sufficient, as gltf may refer to
		// files outside the parent directory, but that is heavily discouraged.
		let src_path = PathBuf::from(path);
		println!(
			"cargo:rerun-if-changed={}",
			src_path.parent().unwrap().to_str().unwrap()
		);
		let out_path = out_dir.join(format!("{}.bin.zstd", src_path.file_stem().unwrap().to_str().unwrap()));
		(src_path, out_path)
	});

	{
		profiling::scope!("processing all models");
		model_paths
			.par_iter()
			.map(|(src_path, out_path)| {
				profiling::scope!("processing model", src_path.to_str().unwrap());
				let gltf = Gltf::open(src_path.clone())?;
				let disk = gltf.process()?;
				disk.serialize_compress_to(File::create(out_path).map_err(MeshletError::from)?)
					.map_err(MeshletError::from)?;
				Ok::<(), MeshletError>(())
			})
			.collect::<Result<_, _>>()?;
	}

	{
		profiling::scope!("writing models.rs");
		let mut out = quote! {};
		for (src_path, out_path) in model_paths {
			let name = format_ident!("{}", src_path.file_stem().unwrap().to_str().unwrap());
			let out_path_str = out_path.to_str().unwrap();
			out = quote! {
				#out

				// TODO remove unused
				#[allow(unused)]
				#[allow(non_upper_case_globals)]
				pub const #name: &[u8] = include_bytes!(#out_path_str);
			}
		}
		fs::write(out_dir.join("models.rs"), out.to_string())?;
	}

	Ok(())
}
