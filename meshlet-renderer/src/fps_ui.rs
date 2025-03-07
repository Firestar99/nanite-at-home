use crate::delta_time::DeltaTime;
use egui::{Align2, Context, Id, RichText, Widget};

pub struct FpsUi {
	retain_peroid: f32,
	last_frames: Vec<DeltaTime>,
}

impl FpsUi {
	pub fn new() -> Self {
		Self {
			retain_peroid: 1.,
			last_frames: Vec::new(),
		}
	}

	pub fn retain_peroid(&mut self, retain_peroid: f32) {
		self.retain_peroid = retain_peroid;
	}

	pub fn update(&mut self, delta: DeltaTime) {
		// this is not efficient and will cause a lot of memmove within the vec
		self.last_frames
			.retain(|f| (delta.since_start - f.since_start) < self.retain_peroid);
		self.last_frames.push(delta);
	}

	pub fn ui(&mut self, ctx: &Context) {
		egui::Area::new(Id::new("fps_ui"))
			.anchor(Align2::RIGHT_TOP, egui::Vec2::new(0., 0.))
			.interactable(false)
			.fade_in(false)
			.show(ctx, |ui| {
				let frames = self.last_frames.len();
				if frames > 0 {
					let avg_s = self.last_frames.iter().map(|f| f.delta_time).sum::<f32>() / frames as f32;
					let text = RichText::new(format!("{:.0} fps\n{:.3} ms\n", 1. / avg_s, avg_s * 1000.)).strong();
					egui::Label::new(text).extend().ui(ui);
				}
			});
	}
}
