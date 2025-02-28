use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// `winit`'s [`Window`] is technically Send + Sync, but I'm not trusting that. `WindowRef` wraps a [`Window`] and will
/// only give you access if you have the [`ActiveEventLoop`], which is only available on the main thread
/// within the closure of [`EventLoopExecutor::spawn()`].
///
/// [`EventLoopExecutor::spawn()`]: rust_gpu_bindless_winit::event_loop::EventLoopExecutor::spawn
#[derive(Debug, Clone)]
pub struct WindowRef {
	window: Arc<Window>,
}

impl WindowRef {
	pub fn new(window: Arc<Window>) -> Self {
		Self { window }
	}

	pub fn get<'a>(&'a self, _event_loop: &'a ActiveEventLoop) -> &'a Arc<Window> {
		&self.window
	}

	pub unsafe fn get_unchecked(&self) -> &Arc<Window> {
		&self.window
	}
}
