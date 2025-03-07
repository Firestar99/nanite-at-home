//! Hello_world example from egui

use rust_gpu_bindless_egui_examples::run;

pub fn main() {
	let mut app = MyApp::default();
	run(move |ctx| app.update(ctx));
}

struct MyApp {
	name: String,
	age: u32,
}

impl Default for MyApp {
	fn default() -> Self {
		Self {
			name: "Arthur".to_owned(),
			age: 42,
		}
	}
}

impl MyApp {
	fn update(&mut self, ctx: &egui::Context) {
		egui::CentralPanel::default().show(ctx, |ui| {
			ui.heading("My egui Application");
			ui.horizontal(|ui| {
				let name_label = ui.label("Your name: ");
				ui.text_edit_singleline(&mut self.name).labelled_by(name_label.id);
			});
			ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
			if ui.button("Increment").clicked() {
				self.age += 1;
			}
			ui.label(format!("Hello '{}', age {}", self.name, self.age));

			// ui.image(egui::include_image!(
			//     "../../../crates/egui/assets/ferris.png"
			// ));
		});
	}
}
