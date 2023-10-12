use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use smallvec::{SmallVec, smallvec};
use static_assertions::assert_not_impl_any;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::GpuFuture;

use space_engine_common::space::renderer::frame_data::FrameData;

use crate::space::Init;
use crate::space::renderer::frame_in_flight::{FrameInFlight, SeedInFlight};
use crate::space::renderer::frame_in_flight::frame_manager::FrameManager;
use crate::space::renderer::frame_in_flight::uniform::UniformInFlight;

/// `RenderContext` is the main instance of the renderer, talking care of rendering frames and most notable ensuring no
/// data races when multiple frames are currently in flight.
pub struct RenderContext {
	pub init: Arc<Init>,
	pub seed: SeedInFlight,
	pub output_format: Format,
	pub frame_data_uniform: UniformInFlight<FrameData>,
	_private: PhantomData<()>,
}

impl RenderContext {
	pub fn new(init: Arc<Init>, output_format: Format, frames_in_flight: u32) -> (Arc<Self>, RenderContextNewFrame) {
		let frame_manager = FrameManager::new(init.clone(), frames_in_flight);
		let seed = frame_manager.seed();
		let render_context = Arc::new(Self {
			frame_data_uniform: UniformInFlight::new(&init, seed, true),
			init,
			seed,
			output_format,
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

impl From<&Arc<RenderContext>> for SeedInFlight {
	#[inline]
	fn from(value: &Arc<RenderContext>) -> Self {
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
	pub fn new_frame<F>(self: &mut Self, output_image: Arc<ImageView>, frame_data: FrameData, f: F)
		where
			F: FnOnce(FrameContext) -> FenceSignalFuture<Box<dyn GpuFuture>>,
	{
		self.frame_manager.new_frame(|frame_in_flight| {
			assert_eq!(output_image.format(), self.render_context.output_format, "ImageView format must match constructed format");
			let extent = output_image.image().extent();
			assert_eq!(extent[2], 1, "must be a 2D image");

			self.render_context.frame_data_uniform.upload(frame_in_flight, frame_data);

			f(FrameContext {
				render_context: self.render_context.clone(),
				frame_in_flight,
				frame_data,
				output_image,
				viewport: Viewport {
					offset: [0f32; 2],
					extent: [extent[0] as f32, extent[1] as f32],
					depth_range: 0f32..=1f32,
				},
				_private: PhantomData::default(),
			})
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
	pub frame_in_flight: FrameInFlight<'a>,
	pub frame_data: FrameData,
	pub output_image: Arc<ImageView>,
	pub viewport: Viewport,
	_private: PhantomData<()>,
}

impl<'a> FrameContext<'a> {
	pub fn viewport_smallvec(&self) -> SmallVec<[Viewport; 2]> {
		smallvec![self.viewport.clone()]
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
		(&value.render_context).into()
	}
}
