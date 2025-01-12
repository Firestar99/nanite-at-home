use space_engine_shader::renderer::lod_selection::{LodSelection, LodType};
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct LodSelector {
	pub lod_level: LodSelection,
}

impl LodSelector {
	pub fn new() -> Self {
		Self {
			lod_level: LodSelection::new_nanite(),
		}
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
			let mut lod_level = self.lod_level.to_i32();
			match code {
				KeyR => lod_level -= 1,
				KeyF => lod_level += 1,
				_ => return,
			}
			self.lod_level = LodSelection::from(i32::clamp(lod_level, -1, 31)).unwrap();
			println!(
				"Lod level: {:?}{}",
				self.lod_level,
				match self.lod_level.lod_type() {
					LodType::Nanite => "",
					LodType::Static => " (or minimum available)",
				}
			);
		}
	}
}
