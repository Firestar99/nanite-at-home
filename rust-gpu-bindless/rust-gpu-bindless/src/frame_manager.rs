use crate::backend::table::FrameGuard;
use crate::descriptor::Bindless;
use crate::frame_in_flight::{FrameInFlight, ResourceInFlight, SeedInFlight};
use std::sync::Arc;
use vulkano::sync::future::{FenceSignalFuture, NowFuture, SemaphoreSignalFuture};
use vulkano::sync::{now, GpuFuture};
use vulkano::{Validated, VulkanError};

pub struct FrameManager {
	bindless: Arc<Bindless>,
	frame_id_mod: u32,
	prev_frame: ResourceInFlight<Option<PrevFrame>>,
}

/// This type will always implement GpuFuture, but it's concrete type may change.
pub type PrevFrameFuture = NowFuture;

struct PrevFrame {
	fence_rendered: FenceSignalFuture<Box<dyn GpuFuture>>,
	_guard: FrameGuard,
}

impl FrameManager {
	pub fn new(bindless: Arc<Bindless>, frames_in_flight: u32) -> Self {
		let seed = SeedInFlight::new(frames_in_flight);
		Self {
			bindless,
			frame_id_mod: seed.frames_in_flight() - 1,
			prev_frame: ResourceInFlight::new(seed, |_| None),
		}
	}

	/// Starts work on a new frame. The function supplied is called with an instance of [`Frame`], containing the
	/// [`FrameInFlight`] as well as providing functionality to flush [`GpuFuture`]s safely. It should return a
	/// (potentially unflushed) [`FenceSignalFuture`] to indicate when the frame has finished rendering. If rendering or
	/// presenting fail, it should return [`None`], which is safe as vulkano will wait for the device to be idle when an
	/// error occurs before returning the Error to the user.
	///
	/// Two conditions are always satisfied by this function to prevent race conditions between CPU and GPU:
	/// * A new frame with the same frame-in-flight id is only started once the last frame with the same frame-in-flight
	///   id has finished rendering on the GPU. This ensures uniform buffers can be safety updated by the CPU without
	///   the GPU still accessing them.
	/// * The next frame does not start rendering on the GPU before the previous has finished. This allows all frames in
	///   flight to share temporary resources needed for rendering, like a depth buffer.
	///
	/// # Impl-Note
	/// * `frame`: the current "new" frame that should be rendered
	/// * `*_prev`: the previous frame that came immediately before this frame
	/// * `*_last`: the last frame with the same frame in flight index, GPU execution of this frame must complete before
	///   this frame can start being recorded due to them sharing resources
	///
	/// [`device_wait_idle`]: vulkano::device::Device::wait_idle
	pub fn new_frame<F>(&mut self, f: F)
	where
		F: FnOnce(&Frame, PrevFrameFuture) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
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
		//	// unlock bindless lock
		// 	drop(last_frame);
		// }

		// FIXME Wait for previous frame to finish on the GPU. Incredibly sad way to do things, as it will stall both
		//  CPU and GPU. But without cloning GpuFutures or at least splitting them into a GPU semaphore and CPU fence
		//  there is nothing we can do to get this right.
		if let Some(prev_frame) = self.prev_frame.index_mut(fif_prev).take() {
			{
				profiling::scope!("wait for GPU");
				prev_frame.fence_rendered.wait(None).unwrap();
			}
			{
				profiling::scope!("cleanup GPU resources");
				// unlock bindless lock
				drop(prev_frame);
			}
		}
		let prev_frame_future = now(self.bindless.device.clone());

		// do the render, write back GpuFuture
		let fence_rendered = {
			let _guard = self.bindless.lock();
			let frame = Frame {
				frame_manager: self,
				fif,
			};
			f(&frame, prev_frame_future)
				.and_then(|fence| frame.flush(fence).ok())
				.map(|fence_rendered| PrevFrame { fence_rendered, _guard })
		};
		*self.prev_frame.index_mut(fif) = fence_rendered;
	}

	#[inline]
	pub fn seed(&self) -> SeedInFlight {
		self.prev_frame.seed()
	}
}

pub struct Frame<'a> {
	pub frame_manager: &'a FrameManager,
	pub fif: FrameInFlight<'a>,
}

impl<'a> Frame<'a> {
	pub fn force_flush(&self) {
		self.frame_manager.bindless.flush();
	}

	pub fn flush<G: GpuFuture>(&self, gpu_future: G) -> Result<G, Validated<VulkanError>> {
		self.force_flush();
		gpu_future.flush()?;
		Ok(gpu_future)
	}

	pub fn then_signal_fence_and_flush<G: GpuFuture>(
		&self,
		gpu_future: G,
	) -> Result<FenceSignalFuture<G>, Validated<VulkanError>> {
		self.force_flush();
		gpu_future.then_signal_fence_and_flush()
	}

	pub fn then_signal_semaphore_and_flush<G: GpuFuture>(
		&self,
		gpu_future: G,
	) -> Result<SemaphoreSignalFuture<G>, Validated<VulkanError>> {
		self.force_flush();
		gpu_future.then_signal_semaphore_and_flush()
	}
}

impl From<&FrameManager> for SeedInFlight {
	fn from(value: &FrameManager) -> Self {
		value.seed()
	}
}
