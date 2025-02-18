use glam::{Vec3, Vec4};
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

pub fn linear_to_srgb_single(linear: f32) -> f32 {
	if linear <= 0.0031308 {
		12.92 * linear
	} else {
		(1.055) * f32::powf(linear, 1.0 / 2.4) - 0.055
	}
}

pub fn linear_to_srgb(linear: Vec3) -> Vec3 {
	Vec3::new(
		linear_to_srgb_single(linear.x),
		linear_to_srgb_single(linear.y),
		linear_to_srgb_single(linear.z),
	)
}

pub fn linear_to_srgb_alpha(linear: Vec4) -> Vec4 {
	Vec4::new(
		linear_to_srgb_single(linear.x),
		linear_to_srgb_single(linear.y),
		linear_to_srgb_single(linear.z),
		linear.w,
	)
}

pub fn srgb_to_linear_single(srgb: f32) -> f32 {
	if srgb <= 0.04045 {
		srgb / 12.92
	} else {
		f32::powf((srgb + 0.055) / (1.055), 2.4)
	}
}

pub fn srgb_to_linear(srgb: Vec3) -> Vec3 {
	Vec3::new(
		srgb_to_linear_single(srgb.x),
		srgb_to_linear_single(srgb.y),
		srgb_to_linear_single(srgb.z),
	)
}

pub fn srgb_to_linear_alpha(srgb: Vec4) -> Vec4 {
	Vec4::new(
		srgb_to_linear_single(srgb.x),
		srgb_to_linear_single(srgb.y),
		srgb_to_linear_single(srgb.z),
		srgb.w,
	)
}
