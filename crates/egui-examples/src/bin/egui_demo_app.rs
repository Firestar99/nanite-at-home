//! egui_demo_app example from egui

use rust_gpu_bindless_egui_examples::main_loop;
use rust_gpu_bindless_winit::event_loop::event_loop_init;

/// this is not using [`rust_gpu_bindless_egui_examples::run`] cause `DemoWindows: !Send`
pub fn main() {
	event_loop_init(|event_loop, events| async move {
		let mut demo = egui_demo_lib::DemoWindows::default();
		main_loop(event_loop, events, move |ctx| demo.ui(ctx)).await.unwrap();
	});
}
