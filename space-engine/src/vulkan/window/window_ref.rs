use std::sync::Arc;

use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

/// Window is technically Send + Sync, but I'm not trusting that.
/// So instead have WindowRef which will only give you Window if you have the EventLoop,
/// which is only available on the main thread using [`run_on_event_loop()`].
///
/// [`run_on_event_loop()`]: crate::vulkan::window::event_loop::run_on_event_loop
#[derive(Debug, Clone)]
pub struct WindowRef {
	window: Arc<Window>,
}

impl WindowRef {
	pub fn new(window: Window) -> Self {
		Self { window: Arc::new(window) }
	}

	pub fn get(&self, _event_loop: &EventLoopWindowTarget<()>) -> &Window {
		&self.window
	}

	pub fn get_arc(&self, _event_loop: &EventLoopWindowTarget<()>) -> &Arc<Window> {
		&self.window
	}
}
