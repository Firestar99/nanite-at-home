use crate::material::screen_space::shared_depth_march::{SharedDepthMarch64, SharedDepthMarchParams};
use crate::renderer::camera::Camera;
use core::cell::Cell;
use glam::{IVec2, UVec2, Vec3, Vec3Swizzles, Vec4};
use spirv_std::image::Image2d;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

#[derive(Copy, Clone)]
pub struct SSSDirectionalLight {
	pub image_size: UVec2,
	/// invocation id
	pub inv_id: u32,
	pub camera: Camera,
	pub light_direction: Vec3,
	pub start_pixel: IVec2,
	/// Length of the screen space ray in pixels.
	pub trace_length: u32,
	/// In meters of camera space
	pub object_thickness: f32,
}

impl SSSDirectionalLight {
	/// The algorithm is written as if depth is `[0, 1]` with 0 being the near plane and 1 the far plane.
	pub fn march<const WG: u32>(&self, depth_image: &Image2d, shared_mem: &[Cell<f32>; 128]) -> f32 {
		let direction = self.camera.transform.normals.transpose() * self.light_direction;
		let mut shared_depth = SharedDepthMarch64::new(
			SharedDepthMarchParams {
				image_size: self.image_size,
				camera: self.camera,
				inv_id: self.inv_id,
				direction: direction.xy(),
				start_pixel: self.start_pixel,
			},
			depth_image,
			shared_mem,
		);

		let ray_origin = shared_depth.origin_depth() - self.object_thickness * 0.5;
		let ray_direction = direction.z / f32::max(direction.x, direction.y);

		// let mut hard_shadow = self.object_thickness * 0.5;
		let mut soft_shadow = Vec4::splat(self.object_thickness * 0.5);

		// skip 0, we don't want to shade ourselves
		shared_depth.advance(depth_image);
		for _ in 1..self.trace_length {
			let sampled_depth = shared_depth.read(depth_image, self.inv_id);
			let ray_depth = ray_origin + ray_direction * shared_depth.cursor() as f32;
			let depth_delta = f32::abs(ray_depth - sampled_depth);

			// hard_shadow = hard_shadow.min(depth_delta);
			match shared_depth.cursor() {
				0 => soft_shadow.x = soft_shadow.x.min(depth_delta),
				1 => soft_shadow.y = soft_shadow.y.min(depth_delta),
				2 => soft_shadow.z = soft_shadow.z.min(depth_delta),
				3 => soft_shadow.w = soft_shadow.w.min(depth_delta),
				_ => (),
			}
		}

		let mut shade: f32 = 1.;
		// shade = shade.min(hard_shadow);
		shade = shade.min(soft_shadow.dot(Vec4::splat(0.25)));
		shade / (self.object_thickness * 0.5)

		// ray_direction * 0.5 + 0.5
	}
}
