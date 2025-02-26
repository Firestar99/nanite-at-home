//! egui_demo_app example from egui

use rust_gpu_bindless_egui_examples::run;

pub fn main() {
	let mut demo = egui_demo_lib::ColorTest::default();
	run(move |ctx| {
		egui::CentralPanel::default().show(&ctx, |ui| demo.ui(ui));
	});
}
