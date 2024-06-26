use crate::meshlet::process::Gltf;
use futures::executor::block_on;
use std::path::PathBuf;

const LANTERN_GLTF_PATH: &str = concat!(
	env!("CARGO_MANIFEST_DIR"),
	"/../asteroids/src/sample_scene/Lantern/glTF/Lantern.gltf"
);

#[test]
fn test_lantern_gltf() {
	let gltf = Gltf::open(PathBuf::from(LANTERN_GLTF_PATH)).unwrap();
	let _scene = block_on(gltf.process()).unwrap();
}
