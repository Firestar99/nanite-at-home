use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::GpuFuture;

use crate::frame_in_flight::{FrameInFlight, ResourceInFlight, SeedInFlight};

pub struct FrameManager {
	frame_id_mod: u32,
	prev_frame: ResourceInFlight<Option<Frame>>,
}

struct Frame {
	fence_rendered: FenceSignalFuture<Box<dyn GpuFuture>>,
}

impl FrameManager {
	pub fn new(frames_in_flight: u32) -> Self {
		let seed = SeedInFlight::new(frames_in_flight);
		Self {
			frame_id_mod: seed.frames_in_flight() - 1,
			prev_frame: ResourceInFlight::new(seed, |_| None),
		}
	}

	/// Starts work on a new frame. The function supplied should return a [`FenceSignalFuture`] to indicate when the frame has finished rendering.
	/// If rendering or especially presenting fails, it should just return [`None`]. On error Vulkano does not create a GpuFuture and instead calls [`device_wait_idle`]
	/// to ensure all resources used by this potentially half way executed command buffer are no longer in flight.
	///
	/// # Impl-Note
	/// * `frame`: the current "new" frame that should be rendered
	/// * `*_prev`: the previous frame that came immediately before this frame
	/// * `*_last`: the last frame with the same frame in flight index,
	/// GPU execution of this frame must complete before this frame can start being recorded due to them sharing resources
	///
	/// [`device_wait_idle`]: vulkano::device::Device::wait_idle
	pub fn new_frame<F>(&mut self, f: F)
	where
		F: FnOnce(FrameInFlight) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
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

		// disabled: see below
		// // Wait for last frame to finish execution, so resources are not contested.
		// // Should only wait when CPU is faster than GPU or vsync.
		// if let Some(last_frame) = self.prev_frame.index_mut(fif).take() {
		// 	last_frame.fence_rendered.wait(None).unwrap();
		// }

		// FIXME Wait for previous frame to finish on the GPU. Incredibly sad way to do things, as it will stall both
		// CPU and GPU. But without cloning GpuFutures or at least splitting them into a GPU semaphore and CPU fence
		// there is nothing we can do to get this right.
		if let Some(prev_frame) = self.prev_frame.index_mut(fif_prev).take() {
			prev_frame.fence_rendered.wait(None).unwrap();
		}

		// do the render, write back GpuFuture
		let fence_rendered = f(fif);
		*self.prev_frame.index_mut(fif) = fence_rendered.map(|fence_rendered| Frame { fence_rendered })
	}

	#[inline(always)]
	pub fn seed(&self) -> SeedInFlight {
		self.prev_frame.seed()
	}
}

impl From<&FrameManager> for SeedInFlight {
	fn from(value: &FrameManager) -> Self {
		value.seed()
	}
}
