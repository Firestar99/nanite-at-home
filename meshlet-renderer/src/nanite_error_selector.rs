use egui::{Ui, Widget};

pub struct NaniteErrorSelector {
	pub error: f32,
}

impl NaniteErrorSelector {
	pub fn new() -> Self {
		Self { error: 1. }
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Nanite Error Scale:");
		egui::Slider::new(&mut self.error, 0.01..=1.).logarithmic(true).ui(ui);
	}
}
