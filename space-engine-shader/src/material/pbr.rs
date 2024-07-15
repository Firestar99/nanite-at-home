use crate::material::light::{DirectionalLight, PointLight};
use core::f32::consts::PI;
use glam::{Vec2, Vec3, Vec4, Vec4Swizzles};
use space_asset::material::pbr::PbrMaterial;
use spirv_std::Sampler;
use vulkano_bindless_shaders::descriptor::reference::Strong;
use vulkano_bindless_shaders::descriptor::Descriptors;

pub fn pbr_material_eval<const D: usize, const P: usize>(
	descriptors: &Descriptors,
	pbr_material: PbrMaterial<Strong>,
	sampler: Sampler,
	world_pos: Vec3,
	normal: Vec3,
	tex_coords: Vec2,
	camera_pos: Vec3,
	point_lights: [PointLight; P],
	directional_lights: [DirectionalLight; D],
	ambient_light: Vec3,
) -> Vec4 {
	let n = normal;
	let v = (camera_pos - world_pos).normalize();

	let base_color: Vec4 = pbr_material.base_color.access(descriptors).sample(sampler, tex_coords)
		* Vec4::from(pbr_material.base_color_factor);
	let albedo = base_color.xyz();
	let alpha = base_color.w;

	let omr: Vec4 = pbr_material.omr.access(descriptors).sample(sampler, tex_coords);
	// let ao = omr.x * pbr_material.occlusion_strength;
	let metallic = omr.y * pbr_material.metallic_factor;
	let roughness = omr.z * pbr_material.roughness_factor;

	let mut lo = Vec3::ZERO;
	for i in 0..point_lights.len() {
		let light = point_lights[i];
		let l = (light.position - world_pos).normalize();
		let distance = (light.position - world_pos).length();
		let attenuation = 1.0 / (distance * distance);
		let radiance = light.color * attenuation;
		lo += evaluate_light(albedo, metallic, roughness, n, v, l, radiance);
	}

	for i in 0..directional_lights.len() {
		let light = directional_lights[i];
		let l = light.direction;
		let radiance = light.color;
		lo += evaluate_light(albedo, metallic, roughness, n, v, l, radiance);
	}

	let ambient = ambient_light * albedo; //  * ao
	let color = ambient + lo;
	let color = color / (color + Vec3::splat(1.0));
	Vec4::from((color, alpha))
}

fn evaluate_light(albedo: Vec3, metallic: f32, roughness: f32, n: Vec3, v: Vec3, l: Vec3, radiance: Vec3) -> Vec3 {
	let h = (v + l).normalize();
	let ndf = distribution_ggx(n, h, roughness);
	let g = geometry_smith(n, v, l, roughness);

	let f0 = Vec3::lerp(Vec3::splat(0.04), albedo, metallic);
	let f = fresnel_schlick(Vec3::dot(h, v).max(0.0), f0);

	let k_specular = f;
	let k_diffuse = (Vec3::splat(1.0) - k_specular) * (1.0 - metallic);

	let numerator = ndf * g * f;
	let denominator = 4.0 * Vec3::dot(n, v).max(0.0) * Vec3::dot(n, l).max(0.0) + 0.0001;
	let specular = numerator / denominator;

	let n_dot_l = Vec3::dot(n, l).max(0.0);
	(k_diffuse * albedo / PI + specular) * radiance * n_dot_l
}

fn fresnel_schlick(cos_theta: f32, f0: Vec3) -> Vec3 {
	f0 + (1.0 - f0) * libm::powf(f32::clamp(1.0 - cos_theta, 0.0, 1.0), 5.0)
}

fn distribution_ggx(n: Vec3, h: Vec3, roughness: f32) -> f32 {
	let a = roughness * roughness;
	let a2 = a * a;
	let n_dot_h = Vec3::dot(n, h).max(0.0);
	let n_dot_h2 = n_dot_h * n_dot_h;

	let num = a2;
	let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
	let denom = PI * denom * denom;

	num / denom
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
	let r = roughness + 1.0;
	let k = (r * r) / 8.0;

	let num = n_dot_v;
	let denom = n_dot_v * (1.0 - k) + k;

	num / denom
}

fn geometry_smith(n: Vec3, v: Vec3, l: Vec3, roughness: f32) -> f32 {
	let n_dot_v = Vec3::dot(n, v).max(0.0);
	let n_dot_l = Vec3::dot(n, l).max(0.0);
	let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
	let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);

	return ggx1 * ggx2;
}
