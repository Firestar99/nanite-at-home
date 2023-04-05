use std::sync::Arc;

use parking_lot::Mutex;

use crate::application_config::ApplicationConfig;

pub struct EngineConfig {
	pub application_config: ApplicationConfig,
}

static CONFIG: Mutex<Option<Arc<EngineConfig>>> = Mutex::new(None);

pub(crate) fn init_config(engine_config: EngineConfig) {
	let mut guard = CONFIG.lock();
	assert!(matches!(guard.as_ref(), None), "EngineConfig already initialized!");
	*guard = Some(Arc::new(engine_config));
}

pub fn get_config() -> Arc<EngineConfig> {
	let guard = CONFIG.lock();
	if let Some(x) = guard.as_ref() {
		x.clone()
	} else {
		panic!("Engine config not yet initialized!");
	}
}
