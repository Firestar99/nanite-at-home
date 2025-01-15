use std::mem::replace;
use std::ops::Deref;
use std::time::Instant;

#[derive(Copy, Clone, Debug, Default)]
pub struct DeltaTime {
	pub delta_time: f32,
	pub since_start: f32,
}

impl Deref for DeltaTime {
	type Target = f32;

	fn deref(&self) -> &Self::Target {
		&self.delta_time
	}
}

#[derive(Copy, Clone, Debug)]
pub struct DeltaTimer {
	start: Instant,
	last: Instant,
}

impl Default for DeltaTimer {
	fn default() -> Self {
		Self::new()
	}
}

impl DeltaTimer {
	pub fn new() -> Self {
		let now = Instant::now();
		Self { start: now, last: now }
	}

	#[allow(clippy::should_implement_trait)]
	pub fn next(&mut self) -> DeltaTime {
		let now = Instant::now();
		DeltaTime {
			delta_time: now.duration_since(replace(&mut self.last, now)).as_secs_f32(),
			since_start: now.duration_since(self.start).as_secs_f32(),
		}
	}
}
