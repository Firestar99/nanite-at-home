use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct NaniteErrorSelector {
	pub error: f32,
}

impl NaniteErrorSelector {
	pub fn new() -> Self {
		Self { error: 1. }
	}

	pub fn handle_input(&mut self, event: &Event<()>) {
		if let Event::WindowEvent {
			event:
				WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							state: ElementState::Pressed,
							physical_key: Code { 0: code },
							..
						},
					..
				},
			..
		} = event
		{
			use winit::keyboard::KeyCode::*;
			let mut error = self.error;
			match code {
				KeyX => error /= 2.,
				KeyC => error *= 2.,
				_ => return,
			}
			self.error = error;
			// self.error = f32::clamp(error, 1., 4096.);
			if self.error < 1. {
				let recip_rounded = self.error.recip().round();
				println!("nanite error: 1/{} = {}", recip_rounded, self.error);
			} else {
				println!("nanite error: {}", self.error);
			}
		}
	}
}
