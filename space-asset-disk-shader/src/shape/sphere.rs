use glam::{Affine3A, Vec3, Vec4, Vec4Swizzles};
use rust_gpu_bindless_macros::BufferStructPlain;

#[derive(Copy, Clone, Debug, Default, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct Sphere(Vec4);

impl Sphere {
	pub fn new(center: Vec3, radius: f32) -> Sphere {
		Self(Vec4::from((center, radius)))
	}

	pub fn merge_spheres_approx(spheres: &[Sphere]) -> Option<Sphere> {
		if spheres.len() == 0 {
			return None;
		}

		let mut center = Vec3::ZERO;
		let mut weight_accum = 0.;
		for sphere in spheres {
			let weight = sphere.radius();
			center += sphere.center() * weight;
			weight_accum += weight;
		}
		center /= weight_accum;

		let radius = spheres
			.iter()
			.map(|s| s.center().distance(center) + s.radius())
			.max_by(|a, b| a.total_cmp(b))
			.unwrap();
		Some(Self::new(center, radius))
	}

	pub fn center(&self) -> Vec3 {
		self.0.xyz()
	}

	pub fn radius(&self) -> f32 {
		self.0.w
	}

	pub fn transform(&self, affine: Affine3A) -> Self {
		Self::new(affine.transform_point3(self.center()), self.radius())
	}
}
