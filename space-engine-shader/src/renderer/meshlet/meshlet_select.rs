use crate::renderer::camera::Camera;
use crate::renderer::compacting_alloc_buffer::{CompactingAllocBufferReader, CompactingAllocBufferWriter};
use crate::renderer::frame_data::FrameData;
use crate::renderer::meshlet::intermediate::{MeshletGroupInstance, MeshletInstance};
use glam::UVec3;
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, Strong, TransientDesc};
use space_asset_shader::meshlet::scene::MeshletScene;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub frame_data: TransientDesc<'a, Buffer<FrameData>>,
	pub scene: TransientDesc<'a, Buffer<MeshletScene<Strong>>>,
	pub compacting_groups_in: CompactingAllocBufferReader<'a, MeshletGroupInstance>,
	pub compacting_instances_out: CompactingAllocBufferWriter<'a, MeshletInstance>,
}

pub const MESHLET_SELECT_WG_SIZE: u32 = 32;

const_assert_eq!(MESHLET_SELECT_WG_SIZE, 32);
#[bindless(compute(threads(32)))]
pub fn meshlet_select_compute(
	#[bindless(descriptors)] mut descriptors: Descriptors,
	#[bindless(param)] params: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
) {
	let group_id = wg_id.x;
	let instance_id = inv_id.x;

	let frame_data = params.frame_data.access(&descriptors).load();
	let group_instance = params.compacting_groups_in.access(&descriptors).read(group_id);
	if instance_id < group_instance.meshlet_cnt {
		let instance = MeshletInstance {
			instance_id: group_instance.instance_id,
			mesh_id: group_instance.mesh_id,
			meshlet_id: group_instance.meshlet_start + instance_id,
		};
		if !cull_meshlet(frame_data.camera, instance) {
			params
				.compacting_instances_out
				.allocate(&mut descriptors)
				.write(&mut descriptors, instance);
		}
	}
}

fn cull_meshlet(_camera: Camera, _instance: MeshletInstance) -> bool {
	false
}
