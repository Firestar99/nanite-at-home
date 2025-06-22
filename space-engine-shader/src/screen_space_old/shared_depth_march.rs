use crate::renderer::camera::Camera;
use crate::screen_space_old::major_axis::MajorAxis;
use crate::utils::lerp::Lerp;
use core::cell::Cell;
use core::mem;
use core::ops::{Deref, DerefMut};
use glam::{ivec2, vec2, IVec2, UVec2, Vec2, Vec3, Vec4};
use spirv_std::arch::{workgroup_memory_barrier_with_group_sync, IndexUnchecked};
use spirv_std::image::Image2d;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

#[derive(Copy, Clone)]
pub struct SharedDepthMarchParams {
	pub image_size: UVec2,
	/// invocation id
	pub inv_id: u32,
	pub camera: Camera,
	pub start_pixel: IVec2,
	pub direction: Vec2,
}

pub struct SharedDepthMarch<'a, const WG: u32, const SHARED_SIZE: usize> {
	// const
	params: SharedDepthMarchParams,
	major_axis: MajorAxis,
	minor_factor: f32,
	shared_mem: &'a [Cell<f32>; SHARED_SIZE],
	origin_depth: f32,

	// stateful
	cursor: u32,
	last_fetch: DepthFetch,
}

pub type SharedDepthMarch128<'a> = SharedDepthMarch<'a, 128, 256>;
pub type SharedDepthMarch64<'a> = SharedDepthMarch<'a, 64, 128>;
pub type SharedDepthMarch32<'a> = SharedDepthMarch<'a, 32, 64>;
pub type SharedDepthMarch16<'a> = SharedDepthMarch<'a, 16, 32>;
pub type SharedDepthMarch8<'a> = SharedDepthMarch<'a, 8, 16>;

impl<'a, const WG: u32, const SHARED_SIZE: usize> Deref for SharedDepthMarch<'a, WG, SHARED_SIZE> {
	type Target = SharedDepthMarchParams;

	fn deref(&self) -> &Self::Target {
		&self.params
	}
}

impl<'a, const WG: u32, const SHARED_SIZE: usize> DerefMut for SharedDepthMarch<'a, WG, SHARED_SIZE> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.params
	}
}

impl<'a, const WG: u32, const SHARED_SIZE: usize> SharedDepthMarch<'a, WG, SHARED_SIZE> {
	fn fetch_depth_bounds_check(&self, depth_image: &Image2d, coord: IVec2) -> f32 {
		if 0 < coord.x && coord.x < self.image_size.x as i32 && 0 < coord.y && coord.y < self.image_size.y as i32 {
			Vec4::from(depth_image.fetch(coord)).x
		} else {
			1.
		}
	}

	fn fetch(&self, depth_image: &Image2d, i: u32) -> DepthFetch {
		let major = i as i32;
		let minor = major as f32 * self.minor_factor;
		let pixel_lower = self.start_pixel + self.major_axis * ivec2(major, f32::floor(minor + 0.5) as i32);
		let depth_lower = self.fetch_depth_bounds_check(depth_image, pixel_lower);
		let depth_upper = self.fetch_depth_bounds_check(depth_image, pixel_lower + self.major_axis * ivec2(0, 1));
		let depth_factor = minor.fract();

		let pixel = self.start_pixel.as_vec2() + self.major_axis * vec2(major as f32, minor);
		let fragment_pos = pixel / self.image_size.as_vec2();
		let camera = self.camera;
		DepthFetch {
			fragment_pos,
			camera,
			depth_lower,
			depth_upper,
			depth_factor,
		}
	}

	fn shared(&self, index: u32) -> &Cell<f32> {
		// Safety: assert in march() ensures len == SHARED_MEM_SIZE and we modulo by that, so no oob can happen
		unsafe { self.shared_mem.index_unchecked((index % (WG * 2)) as usize) }
	}

	pub fn new(
		params: SharedDepthMarchParams,
		depth_image: &Image2d,
		shared_mem: &'a [Cell<f32>; SHARED_SIZE],
	) -> Self {
		assert!(SHARED_SIZE == WG as usize * 2);

		let major_axis = MajorAxis::new(params.direction);
		let minor_factor = major_axis.minor_factor(params.direction);
		let mut this = Self {
			params,
			major_axis,
			minor_factor,
			shared_mem,
			origin_depth: 0.,
			cursor: 0,
			last_fetch: DepthFetch::uninit(),
		};
		this.init(depth_image);
		this
	}

	fn init(&mut self, depth_image: &Image2d) {
		let fetch_0 = self.fetch(depth_image, self.inv_id);
		self.last_fetch = self.fetch(depth_image, self.inv_id + WG);
		self.origin_depth = fetch_0.resolve().z;
		self.shared(self.inv_id).set(self.origin_depth);
		unsafe {
			workgroup_memory_barrier_with_group_sync();
		}
	}

	// Like read advances the internal cursor by 1, but without actually reading the value.
	pub fn advance(&mut self, depth_image: &Image2d) {
		self.cursor = self.cursor + 1;
		if self.cursor % WG == 0 {
			unsafe {
				workgroup_memory_barrier_with_group_sync();
				let i = self.cursor + self.inv_id;
				let new_fetch = self.fetch(depth_image, i + WG * 2);
				let prev_fetch = mem::replace(&mut self.last_fetch, new_fetch);
				self.shared(i + WG).set(prev_fetch.resolve().z);
				workgroup_memory_barrier_with_group_sync();
			}
		}
	}

	/// Read the next depth value plus an offset, advancing the internal cursor by 1.
	/// The offset must not be larger than [`Self::max_read_offset`].
	pub fn read(&mut self, depth_image: &Image2d, offset: u32) -> f32 {
		assert!(offset < Self::max_read_offset());

		let depth = self.shared(self.cursor + offset).get();
		self.advance(depth_image);
		depth
	}

	pub const fn max_read_offset() -> u32 {
		WG
	}

	pub fn origin_depth(&self) -> f32 {
		self.origin_depth
	}

	pub fn cursor(&self) -> u32 {
		self.cursor
	}

	pub fn major_axis(&self) -> MajorAxis {
		self.major_axis
	}

	pub fn minor_factor(&self) -> f32 {
		self.minor_factor
	}
}

#[derive(Copy, Clone)]
struct DepthFetch {
	fragment_pos: Vec2,
	camera: Camera,
	depth_lower: f32,
	depth_upper: f32,
	depth_factor: f32,
}

impl DepthFetch {
	/// Values are senseless. This exists purely to not have to use option
	fn uninit() -> Self {
		Self {
			fragment_pos: Default::default(),
			camera: Camera {
				perspective: Default::default(),
				perspective_inverse: Default::default(),
				transform: Default::default(),
			},
			depth_lower: Default::default(),
			depth_upper: Default::default(),
			depth_factor: Default::default(),
		}
	}
}

impl DepthFetch {
	fn resolve(&self) -> Vec3 {
		let depth = f32::lerp(self.depth_lower, self.depth_upper, self.depth_factor);
		let position = self.camera.reconstruct_from_depth(self.fragment_pos, depth);
		position.camera_space
	}
}
