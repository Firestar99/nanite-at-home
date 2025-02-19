use crate::platform::EguiBindlessPlatform;
use egui::{Context, RawInput, ViewportId};
use egui_winit::State;
use rust_gpu_bindless::generic::descriptor::Bindless;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;

pub struct EguiBindless<P: EguiBindlessPlatform> {
	bindless: Arc<Bindless<P>>,
	ctx: Context,
	egui_winit: State,
}

impl<P: EguiBindlessPlatform> EguiBindless<P> {
	pub fn new<T: 'static>(bindless: Arc<Bindless<P>>, event_loop: &ActiveEventLoop) -> Self {
		let ctx = Context::default();
		let max_texture_side = unsafe { bindless.platform.max_image_dimensions_2d() };
		let egui_winit = State::new(
			ctx.clone(),
			ViewportId::ROOT,
			event_loop,
			None,
			None,
			Some(max_texture_side as usize),
		);
		Self {
			bindless,
			ctx,
			egui_winit,
		}
	}

	pub fn run(&mut self, run_ui: impl FnMut(&Context)) {
		let output = self.ctx.run(RawInput::default(), run_ui);

		// output.
		let vec = self.ctx.tessellate(output.shapes, output.pixels_per_point);
	}
}
