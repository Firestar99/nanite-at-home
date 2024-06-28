use futures::executor::block_on;
use space_asset_pipeline::meshlet::process::Gltf;
use std::env;
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};

const GLTF_MODEL_PATH: &str = concat!(
	env!("CARGO_MANIFEST_DIR"),
	"/src/sample_scene/Lantern/glTF/Lantern.gltf"
);
// const GLTF_MODEL_PATH: &str = "../../models/glTF-Sample-Assets/Models/Triangle/glTF/Triangle.gltf";
// const GLTF_MODEL_PATH: &str = "../../models/glTF-Sample-Assets/Models/Box/glTF/Box.gltf";
// const GLTF_MODEL_PATH: &str = "../../models/bistro/export/Bistro.gltf";
// const GLTF_MODEL_PATH: &str = "../../models/glTF-Sample-Assets/Models/CarbonFibre/glTF/CarbonFibre.gltf";
// const GLTF_MODEL_PATH: &str = "../../models/glTF-Sample-Assets/Models/Sponza/glTF/Sponza.gltf";

fn main() -> Result<(), Box<dyn Error>> {
	// Rerun build script if dir containing gltf has changed. That is technically not sufficient, as gltf may refer to
	// files outside the parent directory, but that is heavily discouraged.
	let in_path = PathBuf::from(GLTF_MODEL_PATH);
	println!("cargo:rerun-if-changed={}", in_path.parent().unwrap().to_str().unwrap());

	let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("TestScene.bin.zstd");
	println!("cargo:rustc-env=TestScenePath={}", out_path.to_str().unwrap());

	let gltf = Gltf::open(in_path)?;
	let disk = block_on(gltf.process())?;
	disk.serialize_compress_to(File::create(out_path)?)?;

	Ok(())
}
