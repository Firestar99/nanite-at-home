use std::sync::Arc;

use vulkano::sync::{GpuFuture, now};
use vulkano::sync::future::FenceSignalFuture;

use crate::space::Init;
use crate::space::renderer::frame_in_flight::{FrameInFlight, SeedInFlight};
use crate::space::renderer::frame_in_flight::resource::ResourceInFlight;

pub struct FrameManager {
	init: Arc<Init>,
	frame_id_mod: u32,
	prev_frame: ResourceInFlight<Option<Frame>>,
}

struct Frame {
	fence_rendered: FenceSignalFuture<Box<dyn GpuFuture>>,
}

impl FrameManager {
	pub fn new(init: Arc<Init>, frames_in_flight: u32) -> Self {
		let seed = SeedInFlight::new(frames_in_flight);
		Self {
			frame_id_mod: 0,
			prev_frame: ResourceInFlight::new(seed, |_| None),
			init,
		}
	}

	/// starts work on a new frame
	///
	/// # Impl-Note
	/// * `frame`: the current "new" frame that should be rendered
	/// * `*_prev`: the previous frame that came immediately before this frame
	/// * `*_last`: the last frame with the same frame in flight index,
	/// GPU execution of this frame must complete before this frame can start being recorded due to them sharing resources
	pub fn new_frame<F>(&mut self, f: F)
		where
			F: FnOnce(FrameInFlight, &dyn GpuFuture) -> FenceSignalFuture<Box<dyn GpuFuture>>,
	{
		// SAFETY: this function ensures the FramesInFlight are never launched concurrently
		let fif;
		let fif_prev;
		unsafe {
			let frame_id_prev = self.frame_id_mod;
			let frame_id = (frame_id_prev + 1) % self.seed().frames_in_flight();
			self.frame_id_mod = frame_id;
			fif = FrameInFlight::new(self.seed(), frame_id);
			fif_prev = FrameInFlight::new(self.seed(), frame_id_prev);
		}

		// Wait for last frame to finish execution, so resources are not contested.
		// Should only wait when CPU is faster than GPU or vsync.
		if let Some(last_frame) = self.prev_frame.index_mut(fif).take() {
			last_frame.fence_rendered.wait(None).unwrap();
		}

		// get the prev frame, this frame should wait with bulk rendering on prev frame to finish
		let frame_prev = self.prev_frame.index(fif_prev);
		let mut future_now = None;
		let future_prev: &dyn GpuFuture = match frame_prev {
			None => {
				// this is only needed on the first frame, so keep the allocation optional
				future_now.insert(now(self.init.device.clone()))
			}
			Some(frame_prev) => {
				&frame_prev.fence_rendered
			}
		};

		// do the render, write back GpuFuture
		let fence_rendered = f(fif, future_prev);
		*self.prev_frame.index_mut(fif) = Some(Frame {
			fence_rendered
		})
	}

	#[inline(always)]
	pub fn seed(&self) -> SeedInFlight {
		self.prev_frame.seed()
	}
}
