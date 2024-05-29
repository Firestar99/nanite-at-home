use smol::fs;
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::cache::{PipelineCache, PipelineCacheCreateInfo};

const PIPELINE_CACHE_FILE_PATH: &str = "pipeline_cache.bin";

#[derive(Clone)]
pub struct SpacePipelineCache(pub Arc<PipelineCache>);

impl SpacePipelineCache {
	pub async fn new(device: Arc<Device>) -> Self {
		let initial_data = fs::read(PIPELINE_CACHE_FILE_PATH).await.unwrap_or_else(|_| Vec::new());

		let pipeline_cache = unsafe {
			PipelineCache::new(
				device,
				PipelineCacheCreateInfo {
					initial_data,
					..PipelineCacheCreateInfo::default()
				},
			)
		}
		.unwrap();
		Self(pipeline_cache)
	}

	pub async fn write(&self) -> io::Result<()> {
		let data = self.get_data().unwrap();
		fs::write(PIPELINE_CACHE_FILE_PATH, data).await
	}
}

impl Deref for SpacePipelineCache {
	type Target = Arc<PipelineCache>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<SpacePipelineCache> for Arc<PipelineCache> {
	fn from(value: SpacePipelineCache) -> Self {
		value.0
	}
}
