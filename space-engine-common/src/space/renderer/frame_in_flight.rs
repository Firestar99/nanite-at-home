//! # Frame in flight
//! The Frame in flight system consists out of 3 main components:
//! * `SeedInFlight`: The seed is the configuration of the Frame in flight system and ensures different seeds are not mixed or matched.
//! To construct it you must pass the amount of frames that may be in flight at maximum, so that other systems can allocate enough resources
//! to support that many frames in flight. The maximum may also not exceed the `FRAMES_LIMIT` of 3, see Efficiency for why.
//! * `FrameInFlight`: A Frame in flight is effectively the index of the frame that is currently in flight. It is constructed via the
//! `new()` from a seed and the index that it should represent, which is marked unsafe as one needs to ensure two frames in flight with
//! the same index are never executing at the same time.
//! * `ResourceInFlight`: A `ResourceInFlight` is a resource that is allocated once per frame that may be in flight at the same time.
//! `FrameInFlight` may be used to index into the `ResourceInFlight` to get the resource to be used for that particular frame. The
//! Resources themselves are stored contiguously inside the `ResourceInFlight` type, and not separately on heap.
//!
//! # Efficiency and Safety
//! First each component stores the seed it was constructed with and checks that it is accessed with the seed it was constructed with,
//! so that no mixing of different seeds may occur. This allows the following conditions to always be true:
//!
//! * `SeedInFlight.frames_in_flight <= FRAMES_LIMIT` checked during Seed construction
//! * `FrameInFlight.index < SeedInFlight.frames_in_flight` checked during `FrameInFlight` construction
//!
//! This has two main advantages:
//! * As indices are always checked during construction, indexing of a `ResourceInFlight` with a `FrameInFlight` may happen without
//! checking that the index is in bound.
//! * `ResourceInFlight` has array of it's resource instead of having to heap allocate it's data and thus introduce an indirection.
//! This array has a capacity of `FRAMES_LIMIT` and is the main factor in the limit, as raising the limit will inherently increase
//! the size of all `ResourceInFlight` structs. If the seed uses less than `FRAMES_LIMIT` maximum frames in flight, the remaining
//! slots in the array will remain uninitialized, but still contribute to the size of `ResourceInFlight`.
//!

use core::mem::{MaybeUninit, size_of};
use core::ops::{Index, IndexMut};

use static_assertions::const_assert_eq;

pub const FRAMES_LIMIT: u32 = 3;

/// The index of a frame that is in flight. See [mod](self) for docs.
#[derive(Copy, Clone)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameInFlight {
	seed: SeedInFlight,
	index: u8,
}
const_assert_eq!(size_of::<FrameInFlight>(), 4);

impl FrameInFlight {
	/// SAFETY: index has to be valid
	unsafe fn new(seed: SeedInFlight, index: u32) -> Self {
		assert!(index < seed.frames_in_flight());
		Self {
			seed,
			index: index as u8,
		}
	}
}

impl From<FrameInFlight> for usize {
	fn from(value: FrameInFlight) -> Self {
		value.index as usize
	}
}

impl From<FrameInFlight> for u32 {
	fn from(value: FrameInFlight) -> Self {
		value.index as u32
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
		assert!(frames_in_flight <= FRAMES_LIMIT, "frames_in_flight_max of {} is over FRAMES_IN_FLIGHT_LIMIT {}", frames_in_flight, FRAMES_LIMIT);
		Self {
			seed: seed.to_ne_bytes(),
			// conversion will always succeed with assert above
			frames_in_flight: frames_in_flight as u8,
		}
	}

	#[must_use]
	fn frames_in_flight(&self) -> u32 {
		self.frames_in_flight as u32
	}

	/// for testing only
	#[must_use]
	#[allow(dead_code)]
	fn seed(&self) -> u16 {
		u16::from_ne_bytes(self.seed)
	}
}


/// A `ResourceInFlight` is a resource that is allocated once per frame that may be in flight at the same time. See [mod](self) for docs.
#[derive(Debug)]
#[repr(C)]
pub struct ResourceInFlight<T> {
	vec: [MaybeUninit<T>; FRAMES_LIMIT as usize],
	seed: SeedInFlight,
}

impl<T> ResourceInFlight<T> {
	#[must_use]
	pub fn from_array<const N: usize>(seed: SeedInFlight, resources: [T; N]) -> Self {
		// implies that N < FRAMES_LIMIT
		assert_eq!(seed.frames_in_flight(), N.try_into().unwrap());
		let mut iter = resources.into_iter();
		Self::from_function(seed, |_| iter.next().unwrap())
	}

	#[must_use]
	pub fn from_function<F>(seed: SeedInFlight, mut f: F) -> Self
		where
			F: FnMut(u32) -> T,
	{
		// just using arrays and a counter, instead of Slices and try_into() array
		let mut i = 0;
		let vec = [(); FRAMES_LIMIT as usize].map(|_| {
			let ret = if i < seed.frames_in_flight() {
				MaybeUninit::new(f(i))
			} else {
				MaybeUninit::uninit()
			};
			i += 1;
			ret
		});
		Self {
			seed,
			vec,
		}
	}
}

impl<T> Index<FrameInFlight> for ResourceInFlight<T> {
	type Output = T;

	#[must_use]
	fn index(&self, index: FrameInFlight) -> &Self::Output {
		assert_eq!(self.seed, index.seed);
		// SAFETY: self.seed.frames_in_flight is the initialized size of the array,
		// the assert above verifies that index is not greater than frames_in_flight
		unsafe {
			self.vec.get_unchecked(usize::from(index)).assume_init_ref()
		}
	}
}

impl<T> IndexMut<FrameInFlight> for ResourceInFlight<T> {
	#[must_use]
	fn index_mut(&mut self, index: FrameInFlight) -> &mut Self::Output {
		assert_eq!(self.seed, index.seed);
		// SAFETY: self.seed.frames_in_flight is the initialized size of the array,
		// the assert above verifies that index is not greater than frames_in_flight
		unsafe {
			self.vec.get_unchecked_mut(usize::from(index)).assume_init_mut()
		}
	}
}

impl<T: Clone> Clone for ResourceInFlight<T> {
	fn clone(&self) -> Self {
		// SAFETY: Self::from_function() will call f() exactly self.seed.frames_in_flight times
		// and self.seed.frames_in_flight is the initialized size of the array
		unsafe {
			ResourceInFlight::from_function(self.seed, |i| self.vec.get_unchecked(i as usize).assume_init_ref().clone())
		}
	}
}

impl<T> Drop for ResourceInFlight<T> {
	fn drop(&mut self) {
		// SAFETY: self.seed.frames_in_flight is the initialized size of the array
		unsafe {
			for i in 0..self.seed.frames_in_flight() as usize {
				self.vec[i].assume_init_drop();
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::rc::Rc;

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

	#[test]
	fn resource_happy() {
		unsafe {
			for n in 0..FRAMES_LIMIT {
				let seed = SeedInFlight::new(n);
				let resource = ResourceInFlight::from_function(seed, |i| i);

				for i in 0..n {
					let fif = FrameInFlight::new(seed, i);
					assert_eq!(resource[fif], i);
				}
			}
		}
	}

	#[test]
	fn resource_from_array() {
		resource_from_array_n([]);
		resource_from_array_n([42]);
		resource_from_array_n([42, 69]);
		resource_from_array_n([42, 69, -12345]);
	}

	fn resource_from_array_n<const N: usize>(array: [i32; N]) {
		unsafe {
			let seed = SeedInFlight::new(array.len() as u32);
			let resource = ResourceInFlight::from_array(seed, array);

			for (i, value) in array.into_iter().enumerate() {
				let fif = FrameInFlight::new(seed, i as u32);
				assert_eq!(resource[fif], value);
			}
		}
	}

	#[test]
	fn resource_drop() {
		for i in 0..FRAMES_LIMIT {
			let seed = SeedInFlight::new(i);
			let rc = Rc::new(());
			assert_eq!(Rc::strong_count(&rc), 1);

			let resource = ResourceInFlight::from_function(seed, |_| rc.clone());
			assert_eq!(Rc::strong_count(&rc), i as usize + 1);

			drop(resource);
			assert_eq!(Rc::strong_count(&rc), 1);
		}
	}

	#[test]
	fn resource_clone_drop() {
		for i in 0..FRAMES_LIMIT {
			let seed = SeedInFlight::new(i);
			let rc = Rc::new(());
			assert_eq!(Rc::strong_count(&rc), 1);

			let resource = ResourceInFlight::from_function(seed, |_| rc.clone());
			assert_eq!(Rc::strong_count(&rc), i as usize + 1);

			let resource2 = resource.clone();
			assert_eq!(Rc::strong_count(&rc), i as usize * 2 + 1);

			drop(resource);
			assert_eq!(Rc::strong_count(&rc), i as usize + 1);

			drop(resource2);
			assert_eq!(Rc::strong_count(&rc), 1);
		}
	}

	#[test]
	#[should_panic]
	fn resource_count_too_low() {
		let seed = SeedInFlight::new(2);
		let _ = ResourceInFlight::from_array(seed, [1]);
	}

	#[test]
	#[should_panic]
	fn resource_count_too_high() {
		let seed = SeedInFlight::new(1);
		let _ = ResourceInFlight::from_array(seed, [1, 2, 3]);
	}

	#[test]
	#[should_panic]
	fn resource_wrong_seed() {
		let seed = SeedInFlight::new(FRAMES_LIMIT);
		let resource = ResourceInFlight::from_array(seed, [1, 2, 3]);
		let seed2 = SeedInFlight::new(FRAMES_LIMIT);
		let fif2 = unsafe { FrameInFlight::new(seed2, 0) };
		let _ = resource[fif2];
	}
}
