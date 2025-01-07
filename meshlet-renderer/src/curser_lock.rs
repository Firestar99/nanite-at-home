use rust_gpu_bindless_winit::event_loop::EventLoopExecutor;
use rust_gpu_bindless_winit::window_ref::WindowRef;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;
use winit::window::CursorGrabMode;

pub struct CursorLock {
	pub event_loop: EventLoopExecutor,
	pub window: WindowRef,
	pub is_grabbed: bool,
}

impl CursorLock {
	pub fn new(event_loop: EventLoopExecutor, window: WindowRef) -> Self {
		Self {
			event_loop,
			window,
			is_grabbed: false,
		}
	}

	pub fn handle_input(&mut self, event: &Event<()>) {
		if let Event::WindowEvent {
			event:
				WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							state: ElementState::Pressed,
							physical_key: Code {
								0: winit::keyboard::KeyCode::Tab,
							},
							repeat: false,
							..
						},
					..
				},
			..
		} = event
		{
			self.is_grabbed = !self.is_grabbed;
			let grab = self.is_grabbed;
			let window = self.window.clone();
			let _ = self.event_loop.spawn(move |e| {
				let window = window.get(e);
				window.set_cursor_visible(!grab);
				if grab {
					if let Err(_) = window.set_cursor_grab(CursorGrabMode::Confined) {
						window.set_cursor_grab(CursorGrabMode::Locked).ok();
					}
				} else {
					window.set_cursor_grab(CursorGrabMode::None).unwrap();
				}
			});
		}
	}
}
