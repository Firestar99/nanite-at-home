use egui::{SliderClamping, Ui, Widget};
use space_engine_shader::renderer::frame_data::NaniteSettings;
use std::ops::RangeInclusive;

pub struct NaniteErrorSelector {
	pub nanite: NaniteSettings,
}

impl NaniteErrorSelector {
	pub fn new() -> Self {
		Self {
			nanite: NaniteSettings {
				error_threshold: 1.0,
				bounding_sphere_scale: 1.0,
			},
		}
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Nanite settings:");
		slider_with_buttons(&mut self.nanite.error_threshold, 0.1..=10., "error threshold", ui);
		slider_with_buttons(
			&mut self.nanite.bounding_sphere_scale,
			0.1..=10.,
			"bounding sphere scale",
			ui,
		);
		ui.label("bounding sphere scale <1.0 can cause holes in models");
	}
}

fn slider_with_buttons(value: &mut f32, range: RangeInclusive<f32>, text: &str, ui: &mut Ui) {
	egui::Slider::new(value, range)
		.logarithmic(true)
		.clamping(SliderClamping::Never)
		.text(text)
		.ui(ui);

	ui.horizontal(|ui| {
		if ui.button(" /2 ").clicked() {
			*value /= 2.;
		}
		if ui.button(" *2 ").clicked() {
			*value *= 2.;
		}
		if ui.button("reset").clicked() {
			*value = 1.;
		}
	});
}
