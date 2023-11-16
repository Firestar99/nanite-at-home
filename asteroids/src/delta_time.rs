use std::mem::replace;
use std::ops::Deref;
use std::time::Instant;

#[derive(Copy, Clone, Debug)]
pub struct DeltaTime(pub f32);

impl Deref for DeltaTime {
	type Target = f32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<DeltaTime> for f32 {
	fn from(value: DeltaTime) -> Self {
		value.0
	}
}

pub struct DeltaTimeTimer {
	last: Instant,
}

impl DeltaTimeTimer {
	pub fn new() -> Self {
		Self { last: Instant::now() }
	}

	pub fn next(&mut self) -> DeltaTime {
		let now = Instant::now();
		DeltaTime(now.duration_since(replace(&mut self.last, now)).as_secs_f32())
	}
}
