use glam::{vec3, Affine3A};
use space_asset::meshlet::indices::triangle_indices_write_vec;
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMesh};
use space_asset::meshlet::mesh2instance::{MeshletMesh2Instance, MeshletMesh2InstanceCpu};
use space_asset::meshlet::offset::MeshletOffset;
use space_asset::meshlet::scene::{LoadedMeshletSceneDisk, MeshletSceneCpu};
use space_asset::meshlet::vertex::MeshletDrawVertex;
use space_asset::uploader::Uploader;
use space_engine::space::Init;
use std::fs::File;
use std::io;
use std::iter::repeat;
use std::sync::Arc;
use vulkano::buffer::{BufferCreateInfo, BufferUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano_bindless::descriptor::RCDescExt;

#[profiling::function]
pub async fn load_scene(init: &Arc<Init>) -> Vec<Arc<MeshletSceneCpu>> {
	let scene = upload_test_scene(init);

	let mut out = Vec::new();
	// out.push(upload_test_mesh(init));
	out.push(scene.await.unwrap());
	out
}

#[profiling::function]
async fn upload_test_scene(init: &Arc<Init>) -> io::Result<Arc<MeshletSceneCpu>> {
	let disk = unsafe {
		let path = env!("TestScenePath");
		let file = File::open(path)?;
		LoadedMeshletSceneDisk::deserialize_decompress(file)?
	};
	let uploader = Uploader {
		bindless: init.bindless.clone(),
		memory_allocator: init.memory_allocator.clone(),
		cmd_allocator: init.cmd_buffer_allocator.clone(),
		transfer_queue: init.queues.client.transfer.clone(),
	};
	let cpu = disk.deserialize().upload(&uploader).await.unwrap();
	Ok(Arc::new(cpu))
}

#[profiling::function]
fn upload_test_mesh(init: &Arc<Init>) -> Arc<MeshletSceneCpu> {
	let alloc_info = AllocationCreateInfo {
		memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
		..AllocationCreateInfo::default()
	};
	let buffer_info = BufferCreateInfo {
		usage: BufferUsage::STORAGE_BUFFER,
		..BufferCreateInfo::default()
	};

	let quads = (0..31)
		.flat_map(|x| (0..31).map(move |y| vec3(x as f32, y as f32, 0.)))
		.collect::<Vec<_>>();

	let vertices = init
		.bindless
		.buffer()
		.alloc_from_iter(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			quads
				.iter()
				.copied()
				.flat_map(|quad| {
					[
						MeshletDrawVertex::new(quad),
						MeshletDrawVertex::new(quad + vec3(1., 0., 0.)),
						MeshletDrawVertex::new(quad + vec3(0., 1., 0.)),
						MeshletDrawVertex::new(quad + vec3(1., 1., 0.)),
					]
				})
				.collect::<Vec<_>>(),
		)
		.unwrap();

	let indices = init
		.bindless
		.buffer()
		.alloc_from_iter(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			triangle_indices_write_vec(
				repeat([0, 1, 2, 1, 2, 3])
					.take(quads.len())
					.flatten()
					.collect::<Vec<_>>()
					.into_iter(),
			),
		)
		.unwrap();

	let meshlets = init
		.bindless
		.buffer()
		.alloc_from_iter(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			quads.iter().enumerate().map(|(i, _)| MeshletData {
				draw_vertex_offset: MeshletOffset::new(i * 4, 4),
				triangle_offset: MeshletOffset::new(i * 2, 2),
			}),
		)
		.unwrap();

	let mesh = init
		.bindless
		.buffer()
		.alloc_from_data(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			MeshletMesh {
				draw_vertices: vertices.to_strong(),
				triangles: indices.to_strong(),
				meshlets: meshlets.to_strong(),
				num_meshlets: meshlets.len() as u32,
			},
		)
		.unwrap();

	let instances = init
		.bindless
		.buffer()
		.alloc_from_iter(
			init.memory_allocator.clone(),
			buffer_info.clone(),
			alloc_info.clone(),
			(0..1)
				.flat_map(|x| {
					(0..1).flat_map(move |y| {
						(0..4).map(move |z| {
							MeshletInstance::new(Affine3A::from_translation(vec3(
								x as f32 * 31.,
								y as f32 * 31. + 4.,
								z as f32 * 4.,
							)))
						})
					})
				})
				.collect::<Vec<_>>(),
		)
		.unwrap();

	Arc::new(MeshletSceneCpu {
		mesh2instances: Vec::from([MeshletMesh2InstanceCpu {
			mesh2instance: MeshletMesh2Instance {
				mesh: mesh.into(),
				instances: instances.into(),
			},
			num_meshlets: meshlets.len() as u32,
		}]),
	})
}
