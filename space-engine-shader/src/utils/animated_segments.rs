use core::ops::{Add, Mul};

pub trait Lerp: Copy {
	fn lerp(a: Self, b: Self, f: f32) -> Self;
}

impl<T: Copy + Mul<f32, Output = T> + Add<T, Output = T>> Lerp for T {
	fn lerp(a: Self, b: Self, f: f32) -> Self {
		a * f + b * (1. - f)
	}
}

pub struct Segment<T: Lerp> {
	pub time: f32,
	pub t: T,
}

impl<T: Lerp> Segment<T> {
	pub const fn new(time: f32, t: T) -> Self {
		Self { time, t }
	}
}

pub struct AnimatedSegment<'a, T: Lerp> {
	segments: &'a [Segment<T>],
	max_time: f32,
}

impl<'a, T: Lerp> AnimatedSegment<'a, T> {
	pub const fn new(segments: &'a [Segment<T>]) -> Self {
		assert!(segments.len() > 0);
		Self {
			segments,
			max_time: match segments.last() {
				None => panic!(),
				Some(s) => s.time,
			},
		}
	}

	pub fn lerp(&self, time: f32) -> T {
		let time = time % self.max_time;
		let mut prev: Option<&Segment<T>> = None;
		for curr in self.segments {
			if time < curr.time {
				return if let Some(prev) = prev {
					T::lerp(curr.t, prev.t, (time - prev.time) / (curr.time - prev.time))
				} else {
					curr.t
				};
			}
			prev = Some(curr);
		}
		panic!()
	}
}
