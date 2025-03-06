use egui::{SliderClamping, Ui, Widget};
use space_engine_shader::renderer::frame_data::NaniteSettings;

pub struct NaniteErrorSelector {
	pub nanite: NaniteSettings,
}

impl NaniteErrorSelector {
	pub fn new() -> Self {
		Self {
			nanite: NaniteSettings::default(),
		}
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Nanite scale settings:");
		egui::Slider::new(&mut self.nanite.error_threshold, 0.1..=10.)
			.logarithmic(true)
			.clamping(SliderClamping::Never)
			.text("error threshold")
			.ui(ui);
		egui::Slider::new(&mut self.nanite.error_scale, 0.1..=10.)
			.logarithmic(true)
			.clamping(SliderClamping::Never)
			.text("error")
			.ui(ui);
		egui::Slider::new(&mut self.nanite.bounding_sphere_scale, 0.1..=10.)
			.logarithmic(true)
			.clamping(SliderClamping::Never)
			.text("bounding sphere")
			.ui(ui);
		// ui.horizontal(|ui| {
		// 	if ui.button(" /2 ").clicked() {
		// 		self.error /= 2.;
		// 	}
		// 	if ui.button(" *2 ").clicked() {
		// 		self.error *= 2.;
		// 	}
		// 	if ui.button("reset").clicked() {
		// 		*self = Self::new();
		// 	}
		// });
	}
}
