use space_engine::generate_application_config;
use space_engine::space::engine_config::EngineConfig;
use space_engine::space::init;
use triangle_example::triangle::triangle_bootup::TRIANGLE_MAIN;

fn main() {
	init(EngineConfig {
		application_config: generate_application_config!()
	}, &TRIANGLE_MAIN);
}
