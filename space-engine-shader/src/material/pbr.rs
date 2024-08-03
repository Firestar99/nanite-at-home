use crate::material::light::{DirectionalLight, PointLight};
use crate::material::radiance::Radiance;
use core::f32::consts::PI;
use core::ops::{Deref, DerefMut};
use glam::{Vec2, Vec3, Vec4, Vec4Swizzles};
use space_asset::material::pbr::PbrMaterial;
use spirv_std::Sampler;
use vulkano_bindless_shaders::descriptor::{AliveDescRef, Descriptors};

/// camera direction unit vector, relative to fragment position
#[derive(Copy, Clone)]
pub struct V(pub Vec3);

impl V {
	pub fn new(world_pos: Vec3, camera_pos: Vec3) -> Self {
		Self((camera_pos - world_pos).normalize())
	}
}

impl Deref for V {
	type Target = Vec3;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for V {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Copy, Clone)]
pub struct SurfaceLocation {
	pub world_pos: Vec3,
	pub tex_coords: Vec2,
	/// vertex normal
	pub normal: Vec3,
	/// camera direction unit vector, relative to fragment position
	pub v: V,
}

impl SurfaceLocation {
	pub fn new(world_pos: Vec3, camera_pos: Vec3, normal: Vec3, tex_coords: Vec2) -> Self {
		Self {
			world_pos,
			tex_coords,
			normal,
			v: V::new(world_pos, camera_pos),
		}
	}
}

#[derive(Copy, Clone)]
pub struct SampledMaterial {
	pub world_pos: Vec3,
	pub v: V,
	pub albedo: Vec3,
	pub alpha: f32,
	pub normal: Vec3,
	pub metallic: f32,
	pub roughness: f32,
}

pub trait PbrMaterialSample {
	fn sample(&self, descriptors: &Descriptors, sampler: Sampler, loc: SurfaceLocation) -> SampledMaterial;
}

impl<R: AliveDescRef> PbrMaterialSample for PbrMaterial<R> {
	/// Sample the material's textures at some texture coordinates.
	/// The sampled values can then be reused for multiple light evaluations.
	fn sample(&self, descriptors: &Descriptors, sampler: Sampler, loc: SurfaceLocation) -> SampledMaterial {
		let base_color: Vec4 =
			self.base_color.access(descriptors).sample(sampler, loc.tex_coords) * Vec4::from(self.base_color_factor);
		let albedo = base_color.xyz();
		let alpha = base_color.w;

		// not yet using normal map
		let normal = loc.normal;

		let omr: Vec4 = self.omr.access(descriptors).sample(sampler, loc.tex_coords);
		// let ao = omr.x * pbr_material.occlusion_strength;
		let metallic = omr.y * self.metallic_factor;
		let roughness = omr.z * self.roughness_factor;

		SampledMaterial {
			world_pos: loc.world_pos,
			v: loc.v,
			albedo,
			alpha,
			normal,
			metallic,
			roughness,
		}
	}
}

impl SampledMaterial {
	pub fn evaluate_directional_light(&self, light: DirectionalLight) -> Radiance {
		let l = light.direction;
		let radiance = light.color;
		self.evaluate_light(l, radiance)
	}

	pub fn evaluate_point_light(&self, light: PointLight) -> Radiance {
		let light_rel = light.position - self.world_pos;
		let l = light_rel.normalize();
		let distance = light_rel.length();
		let attenuation = 1.0 / (distance * distance);
		let radiance = light.color * attenuation;
		self.evaluate_light(l, radiance)
	}

	/// Evaluate the light contribution a light has, not considering visibility.
	///
	/// * `l`: light direction unit vector, relative to fragment position
	/// * `radiance`: radiance the light source is emitting
	pub fn evaluate_light(&self, l: Vec3, radiance: Radiance) -> Radiance {
		let n = self.normal;
		let v = *self.v;

		let h = (v + l).normalize();
		let ndf = distribution_ggx(n, h, self.roughness);
		let g = geometry_smith(n, v, l, self.roughness);

		let f0 = Vec3::lerp(Vec3::splat(0.04), self.albedo, self.metallic);
		let f = fresnel_schlick(Vec3::dot(h, v).max(0.0), f0);

		let k_specular = f;
		let k_diffuse = (Vec3::splat(1.0) - k_specular) * (1.0 - self.metallic);

		let numerator = ndf * g * f;
		let denominator = 4.0 * Vec3::dot(n, v).max(0.0) * Vec3::dot(n, l).max(0.0) + 0.0001;
		let specular = numerator / denominator;

		let n_dot_l = Vec3::dot(n, l).max(0.0);
		Radiance((k_diffuse * self.albedo / PI + specular) * radiance.0 * n_dot_l)
	}

	pub fn ambient_light(&self, radiance: Radiance) -> Radiance {
		Radiance(self.albedo * radiance.0)
	}
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
