use space_asset::meshlet::scene::MeshletSceneFile;

pub fn sample_scenes() -> Vec<MeshletSceneFile<'static>> {
	Vec::from([models::Lantern::glTF::Lantern])
}
