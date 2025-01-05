use crate::renderer::compacting_alloc_buffer::{CompactingAllocBufferReader, CompactingAllocBufferWriter};
use crate::renderer::frame_data::FrameData;
use crate::renderer::lod_selection::LodType;
use crate::renderer::meshlet::intermediate::{MeshletGroupInstance, MeshletInstance};
use crate::utils::affine::AffineTranspose;
use glam::UVec3;
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, Strong, TransientDesc};
use space_asset_shader::meshlet::mesh::MeshletMesh;
use space_asset_shader::meshlet::scene::MeshletScene;
use space_asset_shader::shape::sphere::Sphere;
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
	#[bindless(param)] param: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
) {
	let group_id = wg_id.x;
	let instance_id = inv_id.x;

	let frame_data = param.frame_data.access(&descriptors).load();
	let group_instance = param.compacting_groups_in.access(&descriptors).read(group_id);
	if instance_id < group_instance.meshlet_cnt {
		let instance = MeshletInstance {
			instance_id: group_instance.instance_id,
			mesh_id: group_instance.mesh_id,
			meshlet_id: group_instance.meshlet_start + instance_id,
		};
		if !cull_meshlet(&descriptors, frame_data, param.scene, instance) {
			param
				.compacting_instances_out
				.allocate(&mut descriptors)
				.write(&mut descriptors, instance);
		}
	}
}

fn cull_meshlet(
	descriptors: &Descriptors,
	frame_data: FrameData,
	scene: TransientDesc<Buffer<MeshletScene<Strong>>>,
	instance: MeshletInstance,
) -> bool {
	match frame_data.debug_lod_level.lod_type() {
		LodType::Nanite => {
			let scene = scene.access(descriptors).load();
			let mesh: MeshletMesh<Strong> = scene.meshes.access(descriptors).load(instance.mesh_id as usize);
			let m = mesh.meshlet(descriptors, instance.meshlet_id as usize);
			let instance_transform = scene
				.instances
				.access(descriptors)
				.load(instance.instance_id as usize)
				.transform
				.affine;
			let camera_transform = frame_data.camera.transform.affine.transpose();

			let ss_error = Sphere::new(m.bounds.position(), m.error)
				.transform(instance_transform)
				.transform(camera_transform)
				.project_to_screen_area(frame_data.project_to_screen);
			let ss_error_parent = Sphere::new(m.parent_bounds.position(), m.parent_error)
				.transform(instance_transform)
				.transform(camera_transform)
				.project_to_screen_area(frame_data.project_to_screen);

			let error_threshold = frame_data.nanite_error_threshold;
			let draw = ss_error <= error_threshold && ss_error_parent > error_threshold;
			!draw
		}
		LodType::Static => false,
	}
}
