use std::mem::MaybeUninit;

use crate::frame_in_flight::{FrameInFlight, SeedInFlight, FRAMES_LIMIT};

/// A `ResourceInFlight` is a resource that is allocated once per frame that may be in flight at the same time. See [mod](super) for docs.
///
/// # Indexing
/// There are two possible ways to implement indexing into a `ResourceInFlight`:
/// 1. The [`Index`] & [`IndexMut`] trait
/// 2. normal fn's with proper lifetimes, **the chosen option**
///
/// While being able to index into a ResourceInFlight with indexing brackets is nice with the Index trait, it does not
/// allow proper specifying of the lifetimes required. Leaking of [`FrameInFlight`] is prevented with the specified
/// lifetime, but with [`Index::index`] one can use the [`FrameInFlight`] to index a [`ResourceInFlight`]
/// and then leak the `&T` returned. To prevent this the lifetime of the returned `&T` must not exceed not just
/// `&self`, but also the lifetime of [`FrameInFlight`], which unfortunately makes the `fn` incompatible with the
/// [`Index`] trait.
///
/// [`Index`]: core::ops::Index
/// [`IndexMut`]: core::ops::IndexMut
#[derive(Debug)]
#[repr(C)]
pub struct ResourceInFlight<T> {
	vec: [MaybeUninit<T>; FRAMES_LIMIT as usize],
	seed: SeedInFlight,
}

impl<T> ResourceInFlight<T> {
	/// Main way to construct a ResourceInFlight. Calls the supplied function for each frame in flight (fif) the seed has, so that one resources may be created
	/// for each fif. The function receives an instance of [`FrameInFlight`] for it's index so that one may access other `ResourceInFlight` with the same index
	/// one may depend upon for construction.
	#[must_use]
	pub fn new<F>(seed: impl Into<SeedInFlight>, mut f: F) -> Self
	where
		F: FnMut(FrameInFlight) -> T,
	{
		let seed = seed.into();
		// just using arrays and a counter, instead of Slices and try_into() array, as it prevents heap allocation
		let mut i = 0;
		let vec = [(); FRAMES_LIMIT as usize].map(|_| {
			let ret = if i < seed.frames_in_flight() {
				// SAFETY: allows access to other ResourceInFlights it may depend on, but only with the same index
				let fif = unsafe { FrameInFlight::new(seed, i) };
				MaybeUninit::new(f(fif))
			} else {
				MaybeUninit::uninit()
			};
			i += 1;
			ret
		});
		Self { seed, vec }
	}

	#[must_use]
	pub fn new_array<const N: usize>(seed: impl Into<SeedInFlight>, resources: [T; N]) -> Self {
		let seed = seed.into();
		// implies that N < FRAMES_LIMIT
		assert_eq!(seed.frames_in_flight(), N.try_into().unwrap());
		let mut iter = resources.into_iter();
		Self::new(seed, |_| iter.next().unwrap())
	}

	#[inline(always)]
	pub fn seed(&self) -> SeedInFlight {
		self.seed
	}
}

/// See [`ResourceInFlight#Indexing`]
#[allow(clippy::should_implement_trait)]
impl<T> ResourceInFlight<T> {
	#[must_use]
	#[inline]
	pub fn index<'a>(&'a self, index: FrameInFlight<'a>) -> &'a T {
		assert_eq!(self.seed, index.seed());
		// SAFETY: self.seed.frames_in_flight is the initialized size of the array,
		// the assert above verifies that index is not greater than frames_in_flight
		unsafe { self.vec.get_unchecked(index.index()).assume_init_ref() }
	}

	#[must_use]
	#[inline]
	pub fn index_mut<'a>(&'a mut self, index: FrameInFlight<'a>) -> &'a mut T {
		assert_eq!(self.seed, index.seed());
		// SAFETY: self.seed.frames_in_flight is the initialized size of the array,
		// the assert above verifies that index is not greater than frames_in_flight
		unsafe { self.vec.get_unchecked_mut(index.index()).assume_init_mut() }
	}
}

impl<T: Clone> Clone for ResourceInFlight<T> {
	#[must_use]
	fn clone(&self) -> Self {
		// SAFETY: Self::from_function() will call f() exactly self.seed.frames_in_flight times
		// and self.seed.frames_in_flight is the initialized size of the array
		unsafe {
			ResourceInFlight::new(self.seed, |i| {
				self.vec.get_unchecked(i.index()).assume_init_ref().clone()
			})
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

impl<T> From<&ResourceInFlight<T>> for SeedInFlight {
	fn from(value: &ResourceInFlight<T>) -> Self {
		value.seed()
	}
}

#[cfg(test)]
mod tests {
	use std::rc::Rc;

	use super::*;

	#[test]
	fn resource_happy() {
		unsafe {
			for n in 0..FRAMES_LIMIT {
				let seed = SeedInFlight::new(n);
				let resource = ResourceInFlight::new(seed, |i| i.index() as u32);

				for i in 0..n {
					let fif = FrameInFlight::new(seed, i);
					assert_eq!(*resource.index(fif), i);
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
			let resource = ResourceInFlight::new_array(seed, array);

			for (i, value) in array.into_iter().enumerate() {
				let fif = FrameInFlight::new(seed, i as u32);
				assert_eq!(*resource.index(fif), value);
			}
		}
	}

	#[test]
	fn resource_drop() {
		for i in 0..FRAMES_LIMIT {
			let seed = SeedInFlight::new(i);
			let rc = Rc::new(());
			assert_eq!(Rc::strong_count(&rc), 1);

			let resource = ResourceInFlight::new(seed, |_| rc.clone());
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

			let resource = ResourceInFlight::new(seed, |_| rc.clone());
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
		let _ = ResourceInFlight::new_array(seed, [1]);
	}

	#[test]
	#[should_panic]
	fn resource_count_too_high() {
		let seed = SeedInFlight::new(1);
		let _ = ResourceInFlight::new_array(seed, [1, 2, 3]);
	}

	#[test]
	#[should_panic]
	fn resource_wrong_seed() {
		let seed = SeedInFlight::new(FRAMES_LIMIT);
		let resource = ResourceInFlight::new_array(seed, [1, 2, 3]);
		let seed2 = SeedInFlight::new(FRAMES_LIMIT);
		let fif2 = unsafe { FrameInFlight::new(seed2, 0) };
		let _ = *resource.index(fif2);
	}
}
