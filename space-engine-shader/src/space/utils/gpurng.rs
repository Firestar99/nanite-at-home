/// An PCG PRNG that is optimized for GPUs, in that it is fast to evaluate and accepts sequential ids as it's initial state
/// without sacraficing on RNG quality.
///
/// https://www.reedbeta.com/blog/hash-functions-for-gpu-rendering/
/// https://jcgt.org/published/0009/03/02/
pub struct GpuRng(pub u32);

impl GpuRng {
	pub fn new(state: u32) -> GpuRng {
		Self(state)
	}

	pub fn advance(&mut self) -> u32 {
		let state = self.0;
		self.0 = self.0 * 747796405 + 2891336453;
		let word = ((state >> ((state >> 28) + 4)) ^ state) * 277803737;
		(word >> 22) ^ word
	}

	pub fn advance_f32(&mut self) -> f32 {
		const DIVISOR: f32 = 1. / ((1u64 << 32) as f32);
		self.advance() as f32 * DIVISOR
	}
}
