//! egui_demo_app example from egui

use egui::ScrollArea;
use rust_gpu_bindless_egui_examples::run;

pub fn main() {
	let mut demo = egui_demo_lib::ColorTest::default();
	run(move |ctx| {
		egui::CentralPanel::default().show(ctx, |ui| ScrollArea::vertical().show(ui, |ui| demo.ui(ui)));
	});
}
