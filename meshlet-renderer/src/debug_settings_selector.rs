use space_engine_shader::renderer::frame_data::DebugSettings;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct DebugSettingsSelector {
	selected: DebugSettings,
}

impl DebugSettingsSelector {
	pub fn new() -> Self {
		Self {
			selected: DebugSettings::None,
		}
	}

	pub fn set(&mut self, setting: DebugSettings) {
		self.selected = setting;
	}

	pub fn get(&self) -> DebugSettings {
		self.selected
	}

	pub fn handle_input(&mut self, event: &Event<()>) {
		match event {
			Event::WindowEvent {
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
			} => {
				use winit::keyboard::KeyCode::*;
				let mut selected = u32::from(self.selected) as i32;
				match code {
					// KeyT => selected -= 1,
					KeyE => selected += 1,
					_ => {}
				}
				let selected = i32::rem_euclid(selected, DebugSettings::LEN as i32);
				self.set(DebugSettings::try_from(selected as u32).unwrap());
			}
			_ => {}
		}
	}
}
