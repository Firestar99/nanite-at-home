use glam::{Vec3, Vec4, Vec4Swizzles};
use rust_gpu_bindless_macros::BufferStructPlain;

#[derive(Copy, Clone, Debug, Default, BufferStructPlain)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct Sphere(Vec4);

impl Sphere {
	pub fn new(position: Vec3, radius: f32) -> Sphere {
		Self(Vec4::from((position, radius)))
	}

	#[profiling::function]
	pub fn bounding_sphere<I: Iterator<Item = Vec3>>(iter: impl Fn() -> I) -> Option<Self> {
		let a = Self::bounding_sphere_centered(|| iter())?;
		let b = Self::bounding_sphere_aabb(|| iter())?;
		[a, b].into_iter().max_by(|a, b| a.radius().total_cmp(&b.radius()))
	}

	/// Quite crude approximation and nowhere near the state of the art. `iter` must yield the same values every time.
	pub fn bounding_sphere_centered<I: Iterator<Item = Vec3>>(iter: impl Fn() -> I) -> Option<Self> {
		let (mut center, denom) = iter().fold((Vec3::ZERO, 0.), |a, b| (a.0 + b, a.1 + 1.));
		if denom != 0. {
			center /= denom;
			let radius = iter()
				.map(|a| Vec3::length_squared(a - center))
				.max_by(|a, b| a.total_cmp(b))
				.unwrap();
			Some(Self::new(center, radius))
		} else {
			None
		}
	}

	/// Quite crude approximation and nowhere near the state of the art. `iter` must yield the same values every time.
	pub fn bounding_sphere_aabb<I: Iterator<Item = Vec3>>(iter: impl Fn() -> I) -> Option<Self> {
		let (min, max) = iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |a, b| (a.0.min(b), a.1.max(b)));
		if min != Vec3::INFINITY {
			let diff = max - min;
			Some(Self::new(min + diff * 0.5, diff.length()))
		} else {
			None
		}
	}

	pub fn position(&self) -> Vec3 {
		self.0.xyz()
	}

	pub fn radius(&self) -> f32 {
		self.0.w
	}
}
