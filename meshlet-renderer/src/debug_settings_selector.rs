use egui::Ui;
use space_engine_shader::renderer::frame_data::DebugSettings;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::PhysicalKey::Code;

pub struct DebugSettingsSelector {
	pub debug_settings: DebugSettings,
	pub debug_mix: f32,
}

impl Default for DebugSettingsSelector {
	fn default() -> Self {
		Self::new()
	}
}

impl DebugSettingsSelector {
	pub fn new() -> Self {
		Self {
			debug_settings: DebugSettings::None,
			debug_mix: 1.0,
		}
	}

	pub fn debug_mix_adjusted(&self) -> f32 {
		if self.debug_settings == DebugSettings::None {
			0.
		} else {
			self.debug_mix
		}
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
				let mut selected = u32::from(self.debug_settings) as i32;
				match code {
					KeyQ => selected -= 1,
					KeyE => selected += 1,
					_ => return,
				}
				let selected = i32::rem_euclid(selected, DebugSettings::LEN as i32);
				self.debug_settings = DebugSettings::try_from(selected as u32).unwrap();
				println!("DebugSettings: {:?}", self.debug_settings);
			}
			_ => {}
		}
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Debug View:");
		egui::ComboBox::from_id_salt(concat!(file!(), line!()))
			.selected_text(format!("{:?}", self.debug_settings))
			.show_ui(ui, |ui| {
				for x in (0..DebugSettings::LEN).map(|i| DebugSettings::try_from(i).unwrap()) {
					ui.selectable_value(&mut self.debug_settings, x, format!("{:?}", x));
				}
			});
		ui.add_enabled(
			self.debug_settings != DebugSettings::None,
			egui::Slider::new(&mut self.debug_mix, 0. ..=1.).text("color mix"),
		);
	}
}
