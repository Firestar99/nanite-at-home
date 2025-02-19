fn main() {}

fn ui(ctx: &egui::Context) {
	egui::SidePanel::left("left panel").show(ctx, |ui| {
		ui.label("Hello world!");
		if ui.button("Button").clicked() {
			println!("Button clicked!");
		}
	});
}
