use crate::renderer::camera::Camera;
use crate::renderer::compacting_alloc_buffer::CompactingAllocBufferWriter;
use crate::renderer::frame_data::FrameData;
use core::ops::Range;
use glam::UVec3;
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, Strong, TransientDesc};
use space_asset_shader::meshlet::instance::{MeshInstance, MeshletInstance};
use space_asset_shader::meshlet::mesh::MeshletMesh;
use space_asset_shader::meshlet::scene::MeshletScene;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub scene: TransientDesc<'a, Buffer<MeshletScene<Strong>>>,
	pub num_instances: u32,
	pub compacting_alloc_buffer: CompactingAllocBufferWriter<'a, MeshletInstance>,
}

pub const INSTANCE_CULL_WG_SIZE: u32 = 32;

const_assert_eq!(INSTANCE_CULL_WG_SIZE, 32);
#[bindless(compute(threads(32)))]
pub fn instance_cull_compute(
	#[bindless(descriptors)] mut descriptors: Descriptors,
	#[bindless(param)] params: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
) {
	let wg_instance_offset = wg_id.x * INSTANCE_CULL_WG_SIZE;
	let instance_id = wg_instance_offset + inv_id.x;
	if instance_id >= params.num_instances {
		return;
	}

	let frame_data = params.frame_data.access(&descriptors).load();
	let scene = params.scene.access(&descriptors).load();
	let instance = scene.instances.access(&descriptors).load(instance_id as usize);
	if !cull_instance(frame_data.camera, instance) {
		for mesh_id in Range::<u32>::from(instance.mesh_ids) {
			let mesh: MeshletMesh<Strong> = scene.meshes.access(&descriptors).load(mesh_id as usize);
			let lod_ranges = mesh.lod_ranges.access(&descriptors);
			let lod_level = u32::clamp(frame_data.debug_lod_level, 0, mesh.num_lod_ranges - 1) as usize;
			let meshlet_range = lod_ranges.load(lod_level)..lod_ranges.load(lod_level + 1);
			for meshlet_id in meshlet_range {
				let _ = params.compacting_alloc_buffer.allocate(&mut descriptors).write(
					&mut descriptors,
					MeshletInstance {
						instance_id,
						mesh_id,
						meshlet_id,
					},
				);
			}
		}
	}
}

fn cull_instance(_camera: Camera, _instance: MeshInstance) -> bool {
	false
}
