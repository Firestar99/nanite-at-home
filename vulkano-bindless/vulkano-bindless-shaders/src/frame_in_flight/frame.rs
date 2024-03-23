use std::marker::PhantomData;
use std::mem::size_of;

use static_assertions::const_assert_eq;

use crate::frame_in_flight::FRAMES_LIMIT;

/// The index of a frame that is in flight. See [mod](self) for docs.
#[derive(Copy, Clone)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameInFlight<'a> {
	seed: SeedInFlight,
	index: u8,
	phantom: PhantomData<&'a ()>,
}
const_assert_eq!(size_of::<FrameInFlight>(), 4);

impl<'a> FrameInFlight<'a> {
	/// `FrameInFlight` should be handled carefully as it allows access to a resource that may be in flight. To prevent mis-use it is usually constrained
	/// to an invocation of a `Fn` lambda from where it should not escape, enforced with the (unused) lifetime.
	/// Thus the only two ways to create one safely are:
	/// * When creating a [`ResourceInFlight`] with [`ResourceInFlight::new`] where it may be used to access other `ResourceInFlight`s this one may depend
	///   upon for construction.
	/// * Using `FrameManager` from the `space-engine` crate to control when a frame starts and ends.
	///
	/// # Safety
	/// One may not use the `FrameInFlight` to access a Resource that is currently in use.
	#[inline]
	pub unsafe fn new(seed: impl Into<SeedInFlight>, index: u32) -> Self {
		fn inner<'a>(seed: SeedInFlight, index: u32) -> FrameInFlight<'a> {
			assert!(index < seed.frames_in_flight());
			FrameInFlight {
				seed,
				index: index as u8,
				phantom: Default::default(),
			}
		}
		inner(seed.into(), index)
	}

	#[inline(always)]
	pub fn index(&self) -> usize {
		self.index as usize
	}

	#[inline(always)]
	pub fn seed(&self) -> SeedInFlight {
		self.seed
	}
}

impl<'a> From<FrameInFlight<'a>> for usize {
	fn from(value: FrameInFlight) -> Self {
		value.index as usize
	}
}

impl<'a> From<FrameInFlight<'a>> for u32 {
	fn from(value: FrameInFlight) -> Self {
		value.index as u32
	}
}

impl<'a> From<&FrameInFlight<'a>> for SeedInFlight {
	fn from(value: &FrameInFlight<'a>) -> Self {
		value.seed()
	}
}

/// The seed is the configuration of the Frame in flight system and ensures different seeds are not mixed or matched. See [mod](self) for docs.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
pub struct SeedInFlight {
	/// this is a `[u8; 2]` instead of an `u16` to make `FrameInFlight` be 32 bits instead of 48, due to sizeof struct rounding to nearest alignment.
	/// This may allow an alignment of 8 bits, but at the end of the day the most important operation is equality and that should not care.
	seed: [u8; 2],
	frames_in_flight: u8,
}
const_assert_eq!(size_of::<SeedInFlight>(), 3);

impl SeedInFlight {
	#[cfg(not(target_arch = "spirv"))]
	#[must_use]
	pub fn new(frames_in_flight: u32) -> Self {
		use std::sync::atomic::AtomicU16;
		use std::sync::atomic::Ordering::Relaxed;

		static SEED_CNT: AtomicU16 = AtomicU16::new(42);
		let seed = SEED_CNT.fetch_add(1, Relaxed);
		// SAFETY: global atomic counter ensures seeds are unique
		unsafe { Self::assemble(seed, frames_in_flight) }
	}

	/// SAFETY: Only there for internal testing. The seed must never repeat, which `Self::new()` ensures.
	#[must_use]
	unsafe fn assemble(seed: u16, frames_in_flight: u32) -> Self {
		assert!(
			frames_in_flight <= FRAMES_LIMIT,
			"frames_in_flight_max of {} is over FRAMES_IN_FLIGHT_LIMIT {}",
			frames_in_flight,
			FRAMES_LIMIT
		);
		Self {
			seed: seed.to_ne_bytes(),
			// conversion will always succeed with assert above
			frames_in_flight: frames_in_flight as u8,
		}
	}

	/// Should only be used if [`ResourceInFlight::new`] is not sufficient for creating a resource.
	///
	/// # Safety
	/// The returned FrameInFlight may be used to access any ResourceInFlight, of which some indexes which may be in use right now.
	pub unsafe fn iter(&self) -> impl Iterator<Item = FrameInFlight> {
		unsafe {
			let seed = *self;
			(0..self.frames_in_flight()).map(move |frame| FrameInFlight::new(seed, frame))
		}
	}

	#[must_use]
	#[inline(always)]
	pub fn frames_in_flight(&self) -> u32 {
		self.frames_in_flight as u32
	}

	/// for testing only, thus not pub
	#[must_use]
	#[allow(dead_code)]
	#[inline(always)]
	fn seed(&self) -> u16 {
		u16::from_ne_bytes(self.seed)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn seed_happy() {
		unsafe {
			for i in 0..=FRAMES_LIMIT {
				let seed = 0xDEAD + i as u16;
				let s = SeedInFlight::assemble(seed, i);
				assert_eq!(s.frames_in_flight(), i);
				assert_eq!(s.seed(), seed);
				assert_eq!(s, s.clone());
			}

			const SEEDS_TO_CHECK: usize = 5;
			let seeds = [(); SEEDS_TO_CHECK].map(|_| SeedInFlight::new(FRAMES_LIMIT));
			(0..SEEDS_TO_CHECK)
				.flat_map(|a| (0..SEEDS_TO_CHECK).map(move |b| (a, b)))
				.filter(|(a, b)| a != b)
				.for_each(|(a, b)| {
					assert_ne!(seeds[a], seeds[b]);
				})
		}
	}

	#[test]
	#[should_panic]
	fn seed_too_high_fif() {
		unsafe {
			let _ = SeedInFlight::assemble(0, FRAMES_LIMIT + 1);
		}
	}
}
