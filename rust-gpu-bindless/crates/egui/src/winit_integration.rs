use crate::platform::EguiBindlessPlatform;
use crate::renderer::{EguiRenderContext, EguiRenderOutput, EguiRenderer, EguiRenderingError};
use egui::{Context, ViewportId};
use egui_winit::EventResponse;
use std::ops::Deref;
use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// The `EguiWinitContext` represents an egui [`Context`] that is bound to a specific winit [`Window`]. Use
/// [`EguiWinitContext::new`] to create a new `EguiWinitContext` for some [`Context`] and [`Window`] and
/// [`EguiWinitContext::on_window_event`] to accumulate any winit [`WindowEvent`] for this window. Call
/// [`EguiWinitContext::run`] to update the ui, returning a [`EguiRenderOutput`] that may [`EguiRenderOutput::draw`]
/// the ui's geometry onto some image.
pub struct EguiWinitContext<P: EguiBindlessPlatform> {
	window: Arc<Window>,
	render_ctx: EguiRenderContext<P>,
	winit_state: egui_winit::State,
}

impl<P: EguiBindlessPlatform> EguiWinitContext<P> {
	pub fn new(renderer: EguiRenderer<P>, ctx: Context, e: &ActiveEventLoop, window: Arc<Window>) -> Self {
		let max_texture_side = unsafe { renderer.bindless().platform.max_image_dimensions_2d() };
		let mut slf = Self {
			winit_state: egui_winit::State::new(
				ctx.clone(),
				ViewportId::ROOT,
				&e,
				Some(window.scale_factor() as f32),
				e.system_theme(),
				Some(max_texture_side as usize),
			),
			render_ctx: EguiRenderContext::new(renderer, ctx),
			window,
		};
		slf.update_viewport_info(true);
		slf
	}

	pub fn renderer(&self) -> &EguiRenderer<P> {
		self.render_ctx.renderer()
	}

	pub fn render_ctx(&self) -> &EguiRenderContext<P> {
		&self.render_ctx
	}

	/// If this event is a [`WindowEvent`], it will be forwarded to [`Self::on_window_event`].
	#[inline]
	pub fn on_event<T>(&mut self, event: &Event<T>) -> Option<EventResponse> {
		if let Event::WindowEvent { event, .. } = &event {
			Some(self.on_window_event(event))
		} else {
			None
		}
	}

	/// Submit a winit [`WindowEvent`] to be accumulated as input for a later egui [`Self::run`].
	pub fn on_window_event(&mut self, event: &WindowEvent) -> EventResponse {
		self.winit_state.on_window_event(&self.window, event)
	}

	/// Runs the ui using the supplied `run_ui` function: Extracts accumulated input, updates the ui, tessellates
	/// the geometry, uploads it to the GPU and handles any outputs from the ui. Use the returned [`EguiRenderOutput`]
	/// to [`EguiRenderOutput::draw`] the geometry on an image.
	pub fn run(&mut self, run_ui: impl FnMut(&Context)) -> Result<EguiRenderOutput<'_, P>, EguiRenderingError<P>> {
		let scale = self.update_viewport_info(false).recip();
		let raw_input = self.winit_state.take_egui_input(&self.window);
		let (mut render, platform_output) = self.render_ctx.run(raw_input, run_ui)?;
		self.winit_state.handle_platform_output(&self.window, platform_output);
		render.render_scale(scale);
		Ok(render)
	}

	fn update_viewport_info(&mut self, is_init: bool) -> f32 {
		let raw_input = self.winit_state.egui_input_mut();
		let viewport_info = raw_input.viewports.entry(raw_input.viewport_id).or_default();
		egui_winit::update_viewport_info(viewport_info, &self.render_ctx, &self.window, is_init);
		viewport_info.native_pixels_per_point.unwrap()
	}
}

impl<P: EguiBindlessPlatform> Deref for EguiWinitContext<P> {
	type Target = Context;

	fn deref(&self) -> &Self::Target {
		&self.render_ctx
	}
}
