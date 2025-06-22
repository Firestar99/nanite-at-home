#![allow(warnings)]

use crate::screen_space_trace::major_axis::TraceDirection;
use glam::{FloatExt, IVec2, UVec2, UVec3, Vec4};
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Descriptors, Image, Image2d, MutImage, TransientDesc};
use spirv_std::image::sample_with::lod;
use spirv_std::image::ImageWithMethods;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub depth_image: TransientDesc<'a, Image<Image2d>>,
	pub out_image: TransientDesc<'a, MutImage<Image2d>>,
	pub image_size: UVec2,
	pub trace_direction: TraceDirection,
	pub trace_direction_z: f32,
	pub trace_length: u32,
	pub object_thickness: f32,
}

pub const DIRECTIONAL_SHADOWS_WG_SIZE: u32 = 64;
const SHARED_SIZE: usize = DIRECTIONAL_SHADOWS_WG_SIZE as usize * 2;

const_assert_eq!(DIRECTIONAL_SHADOWS_WG_SIZE, 64);
// const_assert_eq!(DIRECTIONAL_SHADOWS_WG_SIZE * 2, 128);
#[bindless(compute(threads(64)))]
pub fn directional_shadows(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(workgroup_id)] wg_id: UVec3,
	#[spirv(local_invocation_id)] inv_id: UVec3,
	// #[spirv(workgroup)] shared: &[f32; SHARED_SIZE],
) {
	let dir = param.trace_direction;
	let start = dir.major_dir() * wg_id.x as i32 * 64 + dir.minor_dir() * wg_id.y as i32;
	let inv_id = inv_id.x;
	let inv_offset = (dir.to_vec2() * inv_id as f32 - 0.5).as_ivec2();

	let out_image = param.out_image.access(&descriptors);
	unsafe {
		out_image.write(start + inv_offset, Vec4::splat(inv_id as f32 / 64. + 1. / 64.));
	}
}

fn fetch_depth(param: &Param<'_>, descriptors: &Descriptors, start: IVec2, offset: i32) {
	let major = start + param.trace_direction.major_dir() * offset;
	let minor_factor = param.trace_direction.minor_factor() * offset as f32;
	let minor_dir = param.trace_direction.minor_dir();

	let depth_fetch = |mut coord: IVec2| {
		let coord = coord.clamp(IVec2::ZERO, param.image_size.as_ivec2());
		param.depth_image.access(descriptors).fetch_with(coord, lod(0)).x
	};
	let depth_floor = depth_fetch(major + (minor_factor.floor() as i32) * minor_dir);
	let depth_ceil = depth_fetch(major + (minor_factor.ceil() as i32) * minor_dir);
	f32::lerp(depth_floor, depth_ceil, minor_factor.fract());
}
