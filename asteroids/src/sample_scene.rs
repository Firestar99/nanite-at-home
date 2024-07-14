use space_asset::meshlet::scene::MeshletSceneCpu;
use space_asset::uploader::Uploader;
use space_engine::renderer::Init;
use std::io;
use std::sync::Arc;

#[profiling::function]
pub async fn load_scene(init: &Arc<Init>) -> Vec<Arc<MeshletSceneCpu>> {
	Vec::from([upload_test_scene(init).await.unwrap()])
}

#[profiling::function]
async fn upload_test_scene(init: &Arc<Init>) -> io::Result<Arc<MeshletSceneCpu>> {
	let scene = models::Lantern::glTF::Lantern.load()?;
	let uploader = Uploader::new(
		init.bindless.clone(),
		init.memory_allocator.clone(),
		init.cmd_buffer_allocator.clone(),
		init.queues.client.transfer.clone(),
	);
	let cpu = scene.root().upload(&uploader).await.unwrap();
	Ok(Arc::new(cpu))
}
