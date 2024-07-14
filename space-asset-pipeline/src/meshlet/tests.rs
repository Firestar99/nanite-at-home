use crate::gltf::Gltf;
use crate::meshlet::merge::{merge_meshlets, MergeStrategy};
use crate::meshlet::process::process_meshlets;
use std::path::Path;

const LANTERN_GLTF_PATH: &str = concat!(
	env!("CARGO_MANIFEST_DIR"),
	"/../models/models/Lantern/glTF/Lantern.gltf"
);

#[test]
fn test_lantern_gltf() -> anyhow::Result<()> {
	let gltf = Gltf::open(Path::new(LANTERN_GLTF_PATH))?;
	let scene = process_meshlets(&gltf)?;
	let _scene = merge_meshlets(scene, MergeStrategy::MergeSingleInstance)?;
	Ok(())
}
