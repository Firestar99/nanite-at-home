use egui::Ui;
use space_engine_shader::renderer::lod_selection::LodSelection;

pub struct LodSelector {
	static_enabled: bool,
	lod_level: u32,
}

impl LodSelector {
	pub fn new() -> Self {
		Self {
			static_enabled: false,
			lod_level: 0,
		}
	}

	pub fn lod_selection(&self) -> LodSelection {
		if self.static_enabled {
			LodSelection::new_static(self.lod_level)
		} else {
			LodSelection::new_nanite()
		}
	}

	pub fn ui(&mut self, ui: &mut Ui) {
		ui.strong("Static LOD level:");
		ui.checkbox(&mut self.static_enabled, "Enable static LOD");
		ui.add_enabled(
			self.static_enabled,
			egui::Slider::new(&mut self.lod_level, 0..=31).text("static LOD"),
		);
	}
}
