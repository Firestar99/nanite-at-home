use space_asset::meshlet::scene::MeshletSceneFile;

pub fn sample_scenes() -> Vec<MeshletSceneFile<'static>> {
	Vec::from([
		models::local::gamescom::bistro::Bistro,
		models::local::gamescom::Sponza::glTF::Sponza,
		models::local::gamescom::San_Miguel::san_miguel,
		models::local::gamescom::rungholt::rungholt,
		models::local::gamescom::lost_empire::lost_empire,
		models::local::gamescom::vokselia_spawn::vokselia_spawn,
		models::local::gamescom::DamagedHelmet::glTF::DamagedHelmet,
		models::Lantern::glTF::Lantern,
		models::local::gamescom::lpshead::head,
		models::local::gamescom::sibenik::sibenik,
	])
}
