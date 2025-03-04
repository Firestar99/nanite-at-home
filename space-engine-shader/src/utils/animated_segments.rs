use crate::utils::lerp::Lerp;

pub struct Segment<T: Copy + Lerp<S = f32>> {
	pub time: f32,
	pub t: T,
}

impl<T: Copy + Lerp<S = f32>> Segment<T> {
	pub const fn new(time: f32, t: T) -> Self {
		Self { time, t }
	}
}

pub struct AnimatedSegment<'a, T: Copy + Lerp<S = f32>> {
	segments: &'a [Segment<T>],
	max_time: f32,
}

impl<'a, T: Copy + Lerp<S = f32>> AnimatedSegment<'a, T> {
	pub const fn new(segments: &'a [Segment<T>]) -> Self {
		assert!(!segments.is_empty());
		Self {
			segments,
			max_time: match segments.last() {
				None => panic!(),
				Some(s) => s.time,
			},
		}
	}

	pub const fn max_time(&self) -> f32 {
		self.max_time
	}

	pub fn lerp(&self, time: f32) -> T {
		let time = time % self.max_time;
		let mut prev: Option<&Segment<T>> = None;
		for curr in self.segments {
			if time < curr.time {
				return if let Some(prev) = prev {
					T::lerp(prev.t, curr.t, (time - prev.time) / (curr.time - prev.time))
				} else {
					curr.t
				};
			}
			prev = Some(curr);
		}
		panic!()
	}
}
