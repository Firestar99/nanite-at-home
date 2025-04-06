use crate::renderer::compacting_alloc_buffer::{CompactingAllocBufferReader, CompactingAllocBufferWriter};
use crate::renderer::frame_data::FrameData;
use crate::renderer::lod_selection::LodType;
use crate::renderer::meshlet::intermediate::{MeshletGroupInstance, MeshletInstance};
use glam::{Affine3A, UVec3, Vec3, Vec3A};
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
			param.compacting_instances_out.allocate(&mut descriptors, instance);
		}
	}
}

fn cull_meshlet(
	descriptors: &Descriptors,
	frame_data: FrameData,
	scene: TransientDesc<Buffer<MeshletScene<Strong>>>,
	instance: MeshletInstance,
) -> bool {
	let scene = scene.access(descriptors).load();
	let mesh: MeshletMesh<Strong> = scene.meshes.access(descriptors).load(instance.mesh_id as usize);
	let m = mesh.meshlet(descriptors, instance.meshlet_id as usize);
	match frame_data.debug_lod_level.lod_type() {
		LodType::Nanite => {
			let instance_transform = scene.instances.access(descriptors).load(instance.instance_id as usize);
			// let transform = |sphere: Sphere, radius: f32| {
			// 	project_to_screen_area(frame_data, instance_transform.world_from_local.affine, sphere, radius)
			// 		* (frame_data.camera.viewport_size.y as f32)
			// };
			// let ss_error = transform(m.bounds, m.error);
			// let ss_error_parent = transform(m.parent_bounds, m.parent_error);
			// let error_threshold = frame_data.nanite.error_threshold;
			// let draw = ss_error <= error_threshold && error_threshold < ss_error_parent;
			// !draw

			// let world_scale = {
			// 	// Scaling a sphere turns it into an ellipsoid, to turn it back into a sphere we place a sphere around it.
			// 	// That is equivalent to multiplying the radius by the axis that is scaled up the most.
			// 	let mat = instance_transform.transform.affine.matrix3;
			// 	f32::max(f32::max(mat.x_axis.length(), mat.y_axis.length()), mat.z_axis.length())
			// };
			let world_scale = 1.0;

			let lod_is_ok = lod_error_is_imperceptible(
				frame_data,
				m.bounds,
				m.error,
				instance_transform.transform.affine,
				world_scale,
			);
			let parent_lod_is_ok = lod_error_is_imperceptible(
				frame_data,
				m.parent_bounds,
				m.parent_error,
				instance_transform.transform.affine,
				world_scale,
			);
			!lod_is_ok || parent_lod_is_ok
		}
		LodType::Static => m
			.lod_level_bitmask
			.contains(frame_data.debug_lod_level.lod_level_bitmask()),
	}
}

// /// https://jglrxavpok.github.io/2024/04/02/recreating-nanite-runtime-lod-selection.html
// pub fn project_to_screen_area(camera: Camera, instance: AffineTransform, sphere: Sphere, error: f32) -> f32 {
// 	#[cfg(target_arch = "spirv")]
// 	use num_traits::float::Float;
// 	if !error.is_finite() {
// 		return error;
// 	}
// 	let position = camera.transform_vertex(instance, sphere.center()).camera_space;
// 	let d2 = position.length_squared();
// 	let camera_proj = camera.perspective.to_cols_array_2d()[1][1];
// 	camera_proj * error / f32::sqrt(d2 - error * error)
// }

/// https://github.com/zeux/meshoptimizer/blob/1e48e96c7e8059321de492865165e9ef071bffba/demo/nanite.cpp#L115
pub fn project_to_screen_area(frame_data: FrameData, world_from_local: Affine3A, sphere: Sphere, error: f32) -> f32 {
	let camera = frame_data.camera;
	let nanite = frame_data.nanite;
	if !error.is_finite() {
		return error;
	}

	let max_scale_factor = {
		// Scaling a sphere turns it into an ellipsoid, to turn it back into a sphere we place a sphere around it.
		// That is equivalent to multiplying the radius by the axis that is scaled up the most.
		let sum = |a: Vec3A| a.x + a.y + a.z;
		let mat = world_from_local.matrix3;
		f32::max(f32::max(sum(mat.x_axis), sum(mat.y_axis)), sum(mat.z_axis))
	};
	let radius = sphere.radius() * max_scale_factor * nanite.bounding_sphere_scale;
	let error = error * max_scale_factor;

	let center_world = world_from_local.transform_point3(sphere.center());
	let d = center_world.distance(camera.view_from_world.translation()) - radius;
	let d = f32::max(d, camera.clip_from_view.to_cols_array_2d()[3][2]);
	let camera_proj = camera.clip_from_view.to_cols_array_2d()[1][1];
	(error / d) * (camera_proj * 0.5)
}

// https://github.com/zeux/meshoptimizer/blob/1e48e96c7e8059321de492865165e9ef071bffba/demo/nanite.cpp#L115
fn lod_error_is_imperceptible(
	frame_data: FrameData,
	lod_sphere: Sphere,
	simplification_error: f32,
	world_from_local: Affine3A,
	world_scale: f32,
) -> bool {
	if !simplification_error.is_finite() {
		return false;
	}

	let camera = frame_data.camera;
	let nanite = frame_data.nanite;

	let sphere_world_space = world_from_local.transform_point3(lod_sphere.center());
	let radius_world_space = world_scale * lod_sphere.radius() * nanite.bounding_sphere_scale;
	let error_world_space = world_scale * simplification_error;

	let mut projected_error = error_world_space;
	// if camera.perspective.to_cols_array_2d()[3][3] != 1.0 {
	// Perspective
	let distance_to_closest_point_on_sphere =
		Vec3::distance(sphere_world_space, camera.transform.translation()) - radius_world_space;
	let distance_to_closest_point_on_sphere_clamped_to_znear = f32::max(
		distance_to_closest_point_on_sphere,
		camera.perspective.to_cols_array_2d()[3][2],
	);
	projected_error /= distance_to_closest_point_on_sphere_clamped_to_znear;
	// }
	projected_error *= camera.perspective.to_cols_array_2d()[1][1] * 0.5;
	projected_error *= camera.viewport_size.x as f32;

	projected_error < nanite.error_threshold
}
