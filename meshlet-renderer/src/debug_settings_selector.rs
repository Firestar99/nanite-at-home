use egui::Ui;
use space_engine_shader::renderer::frame_data::DebugSettings;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct DebugSettingsSelector {
	selected: DebugSettings,
}

impl Default for DebugSettingsSelector {
	fn default() -> Self {
		Self::new()
	}
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

	#[allow(clippy::single_match)]
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
					KeyQ => selected -= 1,
					KeyE => selected += 1,
					_ => return,
				}
				let selected = i32::rem_euclid(selected, DebugSettings::LEN as i32);
				let debug_settings = DebugSettings::try_from(selected as u32).unwrap();
				self.set(debug_settings);
				println!("DebugSettings: {:?}", debug_settings);
			}
			_ => {}
		}
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Debug View:");
		egui::ComboBox::from_id_salt(concat!(file!(), line!()))
			.selected_text(format!("{:?}", self.selected))
			.show_ui(ui, |ui| {
				for x in (0..DebugSettings::MAX_VALUE as u32).map(|i| DebugSettings::try_from(i).unwrap()) {
					ui.selectable_value(&mut self.selected, x, format!("{:?}", x));
				}
			});
	}
}
