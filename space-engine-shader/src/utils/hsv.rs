use glam::{vec3, Vec3};

/// HSV to RGB conversion with smooth color transitions
/// MIT by Inigo Quilez, from https://www.shadertoy.com/view/MsS3Wc
pub fn hsv2rgb_smooth(c: Vec3) -> Vec3 {
	fn modulo(x: Vec3, y: Vec3) -> Vec3 {
		x - y * Vec3::floor(x / y)
	}

	let rgb = Vec3::clamp(
		Vec3::abs(modulo(c.x * 6.0 + vec3(0.0, 4.0, 2.0), Vec3::splat(6.0)) - 3.0) - 1.0,
		Vec3::splat(0.0),
		Vec3::splat(1.0),
	);
	// cubic smoothing
	let rgb = rgb * rgb * (3.0 - 2.0 * rgb);
	c.z * Vec3::lerp(Vec3::splat(1.0), rgb, c.y)
}

/// HSV to RGB conversion
/// MIT by Inigo Quilez, from https://www.shadertoy.com/view/MsS3Wc
pub fn hsv2rgb(c: Vec3) -> Vec3 {
	fn modulo(x: Vec3, y: Vec3) -> Vec3 {
		x - y * Vec3::floor(x / y)
	}

	let rgb = Vec3::clamp(
		Vec3::abs(modulo(c.x * 6.0 + vec3(0.0, 4.0, 2.0), Vec3::splat(6.0)) - 3.0) - 1.0,
		Vec3::splat(0.0),
		Vec3::splat(1.0),
	);
	c.z * Vec3::lerp(Vec3::splat(1.0), rgb, c.y)
}
