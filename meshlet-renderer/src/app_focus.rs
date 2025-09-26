use rust_gpu_bindless_winit::event_loop::EventLoopExecutor;
use rust_gpu_bindless_winit::window_ref::WindowRef;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;
use winit::window::CursorGrabMode;

pub struct AppFocus {
	pub event_loop: EventLoopExecutor,
	pub window: WindowRef,
	pub game_focused: bool,
}

impl AppFocus {
	pub fn new(event_loop: EventLoopExecutor, window: WindowRef) -> Self {
		Self {
			event_loop,
			window,
			game_focused: false,
		}
	}

	pub fn handle_input(&mut self, event: &Event<()>) -> bool {
		if let Event::WindowEvent {
			event:
				WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							state,
							physical_key: Code {
								0: winit::keyboard::KeyCode::Tab,
							},
							repeat,
							..
						},
					..
				},
			..
		} = event
		{
			if *state == ElementState::Pressed && !repeat {
				self.game_focused = !self.game_focused;
				let grab = self.game_focused;
				let window = self.window.clone();
				drop(self.event_loop.spawn(move |e| {
					let window = window.get(e);
					window.set_cursor_visible(!grab);
					if grab {
						if window.set_cursor_grab(CursorGrabMode::Confined).is_err() {
							window.set_cursor_grab(CursorGrabMode::Locked).ok();
						}
					} else {
						window.set_cursor_grab(CursorGrabMode::None).unwrap();
					}
				}));
			}
			true
		} else {
			false
		}
	}
}
