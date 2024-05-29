use std::sync::Arc;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

/// Window is technically Send + Sync, but I'm not trusting that.
/// So instead have WindowRef which will only give you Window if you have the EventLoop,
/// which is only available on the main thread using [`EventLoopExecutor::spawn()`].
///
/// [`EventLoopExecutor::spawn()`]: crate::vulkan::window::event_loop::EventLoopExecutor::spawn
#[derive(Debug, Clone)]
pub struct WindowRef {
	window: Arc<Window>,
}

impl WindowRef {
	pub fn new(window: Window) -> Self {
		Self {
			window: Arc::new(window),
		}
	}

	pub fn get<'a>(&'a self, _event_loop: &'a EventLoopWindowTarget<()>) -> &'a Window {
		&self.window
	}

	pub fn get_arc<'a>(&'a self, _event_loop: &'a EventLoopWindowTarget<()>) -> &'a Arc<Window> {
		&self.window
	}
}
