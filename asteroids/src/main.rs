use asteroids::bootup::MAINLOOP;
use space_engine::generate_application_config;
use space_engine::space::engine_config::EngineConfig;
use space_engine::space::init;

fn main() {
	init(EngineConfig {
		application_config: generate_application_config!()
	}, &MAINLOOP);
}
