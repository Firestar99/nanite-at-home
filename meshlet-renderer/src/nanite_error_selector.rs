use egui::{SliderClamping, Ui, Widget};

pub struct NaniteErrorSelector {
	pub error: f32,
}

impl NaniteErrorSelector {
	pub fn new() -> Self {
		Self { error: 1. }
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Nanite Error:");
		egui::Slider::new(&mut self.error, 0.01..=1.)
			.logarithmic(true)
			.clamping(SliderClamping::Never)
			.text("radius scale")
			.ui(ui);
		ui.horizontal(|ui| {
			if ui.button(" /2 ").clicked() {
				self.error /= 2.;
			}
			if ui.button(" *2 ").clicked() {
				self.error *= 2.;
			}
			if ui.button("reset").clicked() {
				*self = Self::new();
			}
		});
	}
}
