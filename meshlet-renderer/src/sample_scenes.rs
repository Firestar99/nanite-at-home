use space_asset_disk::meshlet::scene::MeshletSceneFile;

pub fn sample_scenes() -> Vec<MeshletSceneFile<'static>> {
	Vec::from(models::ALL_MODELS)
}
