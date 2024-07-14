use crate::renderer::Init;
use smallvec::{smallvec, SmallVec};
use space_engine_shader::renderer::frame_data::FrameData;
use static_assertions::assert_not_impl_any;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::GpuFuture;
use vulkano::ValidationError;
use vulkano_bindless::descriptor::{Buffer, TransientDesc};
use vulkano_bindless::frame_in_flight::upload::UploadInFlight;
use vulkano_bindless::frame_in_flight::{FrameInFlight, SeedInFlight};
use vulkano_bindless::frame_manager::{Frame, FrameManager, PrevFrameFuture};

/// `RenderContext` is the main instance of the renderer, talking care of rendering frames and most notable ensuring no
/// data races when multiple frames are currently in flight.
pub struct RenderContext {
	pub init: Arc<Init>,
	pub seed: SeedInFlight,
	pub frame_data_uniform: UploadInFlight<FrameData>,
	_private: PhantomData<()>,
}

impl RenderContext {
	pub fn new(init: Arc<Init>, frames_in_flight: u32) -> (Arc<Self>, RenderContextNewFrame) {
		let frame_manager = FrameManager::new(init.bindless.clone(), frames_in_flight);
		let seed = frame_manager.seed();
		let frame_data_uniform = UploadInFlight::new(
			&init.bindless,
			init.memory_allocator.clone(),
			seed,
			BufferUsage::STORAGE_BUFFER,
		);
		let render_context = Arc::new(Self {
			init,
			seed,
			frame_data_uniform,
			_private: Default::default(),
		});
		let new_frame = RenderContextNewFrame {
			render_context: render_context.clone(),
			frame_manager,
		};
		(render_context, new_frame)
	}
}

impl From<&RenderContext> for SeedInFlight {
	#[inline]
	fn from(value: &RenderContext) -> Self {
		value.seed
	}
}

/// `RenderContextNewFrame` is like [`RenderContext`], but may not be cloned to guarantee exclusive access, which
/// allows generating [`RenderContextNewFrame::new_frame()`].
pub struct RenderContextNewFrame {
	render_context: Arc<RenderContext>,
	frame_manager: FrameManager,
}

assert_not_impl_any!(RenderContextNewFrame: Clone);

impl RenderContextNewFrame {
	pub fn new_frame<F>(&mut self, viewport: Viewport, frame_data: FrameData, f: F)
	where
		F: FnOnce(&FrameContext, PrevFrameFuture) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
	{
		self.frame_manager.new_frame(|frame, prev_frame_future| {
			// Safety: upload before usage
			let frame_data_desc = unsafe {
				self.render_context
					.frame_data_uniform
					.upload(frame.fif, frame_data)
					.unwrap()
			};

			let render_context = self.render_context.clone();
			f(
				&FrameContext {
					render_context,
					fif: frame.fif,
					frame,
					frame_data,
					frame_data_desc,
					viewport,
					_private: PhantomData,
				},
				prev_frame_future,
			)
		});
	}
}

impl Deref for RenderContextNewFrame {
	type Target = Arc<RenderContext>;

	fn deref(&self) -> &Self::Target {
		&self.render_context
	}
}

/// A `FrameContext` is created once per Frame rendered, containing frame-specific information and access to resources.
pub struct FrameContext<'a> {
	pub render_context: Arc<RenderContext>,
	pub fif: FrameInFlight<'a>,
	pub frame: &'a Frame<'a>,
	pub frame_data: FrameData,
	pub frame_data_desc: TransientDesc<'a, Buffer<FrameData>>,
	pub viewport: Viewport,
	_private: PhantomData<()>,
}

impl<'a> FrameContext<'a> {
	pub fn viewport_smallvec(&self) -> SmallVec<[Viewport; 2]> {
		smallvec![self.viewport.clone()]
	}

	pub fn modify(
		&self,
	) -> impl FnOnce(&mut RecordingCommandBuffer) -> Result<&mut RecordingCommandBuffer, Box<ValidationError>> {
		let viewport = self.viewport.clone();
		move |cmd| cmd.set_viewport(0, smallvec![viewport.clone()])
	}

	#[inline]
	pub fn seed(&self) -> SeedInFlight {
		self.into()
	}
}

impl<'a> Deref for FrameContext<'a> {
	type Target = FrameData;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.frame_data
	}
}

impl<'a> DerefMut for FrameContext<'a> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.frame_data
	}
}

impl<'a> From<&FrameContext<'a>> for SeedInFlight {
	#[inline]
	fn from(value: &FrameContext<'a>) -> Self {
		(&*value.render_context).into()
	}
}
