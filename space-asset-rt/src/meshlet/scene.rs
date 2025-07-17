use crate::image::upload::UploadedImages;
use crate::material::pbr::{PbrMaterials, default_pbr_material, upload_pbr_material};
use crate::meshlet::mesh::upload_mesh;
use crate::upload_traits::ToStrong;
use crate::uploader::{Uploader, deserialize_infallible};
use futures::future::join_all;
use glam::{UVec3, Vec3A};
use rayon::prelude::*;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, RC, RCDesc, RCDescExt,
};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Strong};
use space_asset_disk::meshlet::scene::ArchivedMeshletSceneDisk;
use space_asset_disk::meshlet::stats::MeshletSceneStats;
use space_asset_disk::range::{ArchivedRangeU32, RangeU32};
use space_asset_shader::affine_transform::AffineTransform;
use space_asset_shader::material::pbr::PbrMaterial;
use space_asset_shader::meshlet::instance::MeshInstance;
use space_asset_shader::meshlet::mesh::MeshletMesh;
use space_asset_shader::meshlet::scene::MeshletScene;

#[derive(Clone, Debug)]
pub struct MeshletSceneCpu {
	pub meshes: RCDesc<Buffer<[MeshletMesh<Strong>]>>,
	pub instances: Vec<MeshInstance>,
	pub stats: MeshletSceneStats,
}

#[derive(Clone, Debug)]
pub struct InstancedMeshletSceneCpu {
	pub instance_count: UVec3,
	pub scene: RCDesc<Buffer<MeshletScene<Strong>>>,
	pub num_instances: u32,
}

pub async fn upload_scene(this: &ArchivedMeshletSceneDisk, uploader: &Uploader) -> anyhow::Result<MeshletSceneCpu> {
	profiling::function_scope!();

	let uploaded_images = {
		profiling::scope!("image upload");
		UploadedImages::new(&uploader.bindless, &this.image_storage).await?
	};

	let pbr_materials: Vec<PbrMaterial<RC>> = {
		profiling::scope!("material upload");
		this.pbr_materials
			.par_iter()
			.map(|mat| upload_pbr_material(mat, &uploaded_images))
			.collect::<Result<_, _>>()?
	};
	let pbr_materials = PbrMaterials {
		pbr_materials: &pbr_materials,
		default_pbr_material: &default_pbr_material(&uploaded_images),
	};
	drop(uploaded_images);

	let meshes: Vec<MeshletMesh<RC>> = {
		profiling::scope!("mesh upload");
		join_all(
			this.meshes
				.par_iter()
				.map(|mesh| upload_mesh(mesh, uploader, &pbr_materials))
				.collect::<Vec<_>>(),
		)
		.await
		.into_iter()
		.collect::<Result<_, _>>()?
	};

	let meshes_buffer = {
		profiling::scope!("meshes_buffer upload");
		uploader
			.upload_buffer_iter("meshes", meshes.iter().map(|m| m.to_strong()))
			.await?
	};

	let instances = this
		.instances
		.iter()
		.map(|instance| MeshInstance {
			world_from_local: AffineTransform::new(instance.world_from_local),
			mesh_ids: deserialize_infallible::<ArchivedRangeU32, RangeU32>(&instance.mesh_ids),
		})
		.collect::<Vec<_>>();

	let stats = deserialize_infallible::<_, MeshletSceneStats>(&this.stats);

	Ok(MeshletSceneCpu {
		instances,
		meshes: meshes_buffer,
		stats,
	})
}

impl MeshletSceneCpu {
	pub fn instantiate(&self, bindless: &Bindless, instance_count: UVec3) -> anyhow::Result<InstancedMeshletSceneCpu> {
		profiling::function_scope!();

		let physical_offset = self.stats.source.bounds_max - self.stats.source.bounds_min;
		let total_instances = (instance_count.x * instance_count.y * instance_count.z) as usize * self.instances.len();
		let mut instances = Vec::with_capacity(total_instances);
		for x in 0..instance_count.x {
			for y in 0..instance_count.y {
				for z in 0..instance_count.z {
					let instance_offset = UVec3::new(x, y, z);
					for mut i in self.instances.iter().copied() {
						i.world_from_local.affine.translation +=
							Vec3A::from(physical_offset * instance_offset.as_vec3());
						instances.push(i);
					}
				}
			}
		}
		assert_eq!(instances.len(), total_instances);

		let instances_buffer = bindless.buffer().alloc_shared_from_iter(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER | BindlessBufferUsage::MAP_WRITE,
				name: "instances",
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			instances.into_iter(),
		)?;

		let scene = bindless.buffer().alloc_shared_from_data(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER | BindlessBufferUsage::MAP_WRITE,
				name: "scene",
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			MeshletScene::<Strong> {
				meshes: self.meshes.to_strong(),
				instances: instances_buffer.to_strong(),
			},
		)?;

		Ok(InstancedMeshletSceneCpu {
			scene,
			num_instances: total_instances as u32,
			instance_count,
		})
	}
}
