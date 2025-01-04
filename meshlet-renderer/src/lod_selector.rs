use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct LodSelector {
	pub lod_level: u32,
}

impl LodSelector {
	pub fn new() -> Self {
		Self { lod_level: 0 }
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
			let mut lod_level = self.lod_level as i32;
			match code {
				KeyR => lod_level -= 1,
				KeyF => lod_level += 1,
				_ => {}
			}
			self.lod_level = lod_level.max(0) as u32;
		}
	}
}
