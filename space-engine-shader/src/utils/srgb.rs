use glam::{Vec3, Vec4};

pub fn linear_to_srgb_channel(linear: f32) -> f32 {
	if linear <= 0.0031308 {
		12.92 * linear
	} else {
		(1.055) * libm::powf(linear, 1.0 / 2.4) - 0.055
	}
}

pub fn linear_to_srgb(linear: Vec3) -> Vec3 {
	Vec3::new(
		linear_to_srgb_channel(linear.x),
		linear_to_srgb_channel(linear.y),
		linear_to_srgb_channel(linear.z),
	)
}

pub fn linear_to_srgb_alpha(linear: Vec4) -> Vec4 {
	Vec4::new(
		linear_to_srgb_channel(linear.x),
		linear_to_srgb_channel(linear.y),
		linear_to_srgb_channel(linear.z),
		linear.w,
	)
}

pub fn srgb_to_linear_channel(srgb: f32) -> f32 {
	if srgb <= 0.04045 {
		srgb / 12.92
	} else {
		libm::powf((srgb + 0.055) / (1.055), 2.4)
	}
}

pub fn srgb_to_linear(srgb: Vec3) -> Vec3 {
	Vec3::new(
		srgb_to_linear_channel(srgb.x),
		srgb_to_linear_channel(srgb.y),
		srgb_to_linear_channel(srgb.z),
	)
}

pub fn srgb_to_linear_alpha(srgb: Vec4) -> Vec4 {
	Vec4::new(
		srgb_to_linear_channel(srgb.x),
		srgb_to_linear_channel(srgb.y),
		srgb_to_linear_channel(srgb.z),
		srgb.w,
	)
}
