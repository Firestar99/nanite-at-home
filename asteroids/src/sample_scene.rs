use space_asset::meshlet::scene::{ArchivedMeshletSceneDisk, MeshletSceneCpu};
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
	let disk = unsafe { ArchivedMeshletSceneDisk::deserialize(crate::models::Lantern::glTF::Lantern) };
	let uploader = Uploader {
		bindless: init.bindless.clone(),
		memory_allocator: init.memory_allocator.clone(),
		cmd_allocator: init.cmd_buffer_allocator.clone(),
		transfer_queue: init.queues.client.transfer.clone(),
	};
	let cpu = disk.upload(&uploader).await.unwrap();
	Ok(Arc::new(cpu))
}
