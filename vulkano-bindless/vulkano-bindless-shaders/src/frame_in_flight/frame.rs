use crate::buffer_content::{BufferStruct, Metadata, MetadataCpuInterface};
use crate::frame_in_flight::FRAMES_LIMIT;
use bytemuck_derive::AnyBitPattern;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;

/// ValueType acts as if it always were u16, but on spirv using u16 requires additional capabilities, so we use u32
/// instead. For transfering to the GPU, feel free to bitpack both seed and fif into 16 bits.
#[cfg(not(target_arch = "spirv"))]
type ValueType = u16;
#[cfg(target_arch = "spirv")]
type ValueType = u32;

#[cfg(not(target_arch = "spirv"))]
pub type SeedType = u8;
#[cfg(target_arch = "spirv")]
pub type SeedType = u32;

/// The index of a frame that is in flight. See [mod](self) for docs.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FrameInFlight<'a> {
	value: ValueType,
	phantom: PhantomData<&'a ()>,
}

impl<'a> FrameInFlight<'a> {
	/// `FrameInFlight` should be handled carefully as it allows access to a resource that may be in flight. To prevent mis-use it is usually constrained
	/// to an invocation of a `Fn` lambda from where it should not escape, enforced with the (unused) lifetime.
	/// Thus, the only two ways to create one safely are:
	/// * When creating a [`ResourceInFlight`] with [`ResourceInFlight::new`] where it may be used to access other `ResourceInFlight`s this one may depend
	///   upon for construction.
	/// * Using `FrameManager` from the `space-engine` crate to control when a frame starts and ends.
	///
	/// # Safety
	/// One may not use the `FrameInFlight` to access a Resource that is currently in use.
	#[inline]
	pub unsafe fn new(seed: impl Into<SeedInFlight>, frame_index: u32) -> Self {
		let seed = seed.into();
		assert!(frame_index < seed.frames_in_flight());
		unsafe { Self::new_unchecked(seed, frame_index) }
	}

	#[inline]
	pub(crate) unsafe fn new_unchecked(seed: impl Into<SeedInFlight>, frame_index: u32) -> Self {
		fn inner<'a>(seed: SeedInFlight, index: u32) -> FrameInFlight<'a> {
			let mut value = seed.0;
			value |= (index as ValueType) & 0xFF;
			FrameInFlight {
				value,
				phantom: Default::default(),
			}
		}
		inner(seed.into(), frame_index)
	}

	#[inline]
	pub fn frame_index(&self) -> usize {
		(self.value & 0xF) as usize
	}

	#[inline]
	pub fn seed(&self) -> SeedInFlight {
		// exclude index
		SeedInFlight(self.value & 0xFFF0)
	}
}

impl<'a> Debug for FrameInFlight<'a> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("FrameInFlight")
			.field("seed", &self.seed().seed_u8())
			.field("frames_in_flight", &self.seed().frames_in_flight())
			.field("frame_index", &self.seed().frames_in_flight())
			.finish()
	}
}

impl<'a> From<FrameInFlight<'a>> for usize {
	fn from(value: FrameInFlight) -> Self {
		value.frame_index()
	}
}

impl<'a> From<FrameInFlight<'a>> for u32 {
	fn from(value: FrameInFlight) -> Self {
		value.frame_index() as u32
	}
}

impl<'a> From<&FrameInFlight<'a>> for SeedInFlight {
	fn from(value: &FrameInFlight<'a>) -> Self {
		value.seed()
	}
}

#[allow(clippy::unnecessary_cast)]
unsafe impl<'a> BufferStruct for FrameInFlight<'a> {
	type Transfer = FrameInFlightTransfer;

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		FrameInFlightTransfer {
			value: self.value as u32,
		}
	}

	unsafe fn read(from: Self::Transfer, _meta: Metadata) -> Self {
		FrameInFlight {
			value: from.value as ValueType,
			phantom: PhantomData {},
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct FrameInFlightTransfer {
	value: u32,
}

/// The seed is the configuration of the Frame in flight system and ensures different seeds are not mixed or matched. See [mod](self) for docs.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct SeedInFlight(ValueType);

#[allow(clippy::unnecessary_cast)]
impl SeedInFlight {
	#[cfg(not(target_arch = "spirv"))]
	#[must_use]
	pub fn new(frames_in_flight: u32) -> Self {
		use core::sync::atomic::AtomicU8;
		use core::sync::atomic::Ordering::Relaxed;

		static SEED_CNT: AtomicU8 = AtomicU8::new(42);
		let seed = SEED_CNT.fetch_add(1, Relaxed);
		// SAFETY: global atomic counter ensures seeds are unique
		unsafe { Self::assemble(seed, frames_in_flight) }
	}

	/// # Safety
	/// Only there for internal testing. The seed must never repeat, which `Self::new()` ensures.
	#[must_use]
	pub unsafe fn assemble(seed: SeedType, frames_in_flight: u32) -> Self {
		assert!(frames_in_flight != 0, "frames_in_flight must not be 0");
		assert!(
			frames_in_flight <= FRAMES_LIMIT,
			"frames_in_flight of {} is over FRAMES_LIMIT {}",
			frames_in_flight,
			FRAMES_LIMIT
		);
		unsafe { Self::assemble_unchecked(seed, frames_in_flight) }
	}

	#[must_use]
	pub(crate) unsafe fn assemble_unchecked(seed: SeedType, frames_in_flight: u32) -> Self {
		let mut out = 0;
		out |= ((seed as ValueType) & 0xFF) << 8;
		out |= (((frames_in_flight - 1) as ValueType) & 0xF) << 4;
		Self(out)
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
	#[inline]
	pub fn frames_in_flight(&self) -> u32 {
		((self.0 >> 4) & 0xF) as u32 + 1
	}

	#[must_use]
	#[inline]
	fn seed_u8(&self) -> SeedType {
		((self.0 >> 8) & 0xFF) as SeedType
	}
}

impl Debug for SeedInFlight {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("SeedInFlight")
			.field("seed", &self.seed_u8())
			.field("frames_in_flight", &self.frames_in_flight())
			.finish()
	}
}

#[allow(clippy::unnecessary_cast)]
unsafe impl BufferStruct for SeedInFlight {
	type Transfer = SeedInFlightTransfer;

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		SeedInFlightTransfer(self.0 as u32)
	}

	unsafe fn read(from: Self::Transfer, _meta: Metadata) -> Self {
		SeedInFlight(from.0 as ValueType)
	}
}

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct SeedInFlightTransfer(u32);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn seed_bit_packing() {
		unsafe {
			for i in 1..=FRAMES_LIMIT {
				let seed = 0xDE + i as u8;
				let s = SeedInFlight::assemble(seed, i);
				assert_eq!(s.frames_in_flight(), i);
				assert_eq!(s.seed_u8(), seed);
				assert_eq!(s, s.clone());
			}
		}
	}

	#[test]
	fn seed_distinct() {
		const SEEDS_TO_CHECK: usize = 5;
		let seeds = [(); SEEDS_TO_CHECK].map(|_| SeedInFlight::new(FRAMES_LIMIT));
		(0..SEEDS_TO_CHECK)
			.flat_map(|a| (0..SEEDS_TO_CHECK).map(move |b| (a, b)))
			.filter(|(a, b)| a != b)
			.for_each(|(a, b)| {
				assert_ne!(seeds[a], seeds[b]);
			});
	}

	#[test]
	#[should_panic]
	fn seed_too_high_fif() {
		unsafe {
			let _ = SeedInFlight::assemble(0, FRAMES_LIMIT + 1);
		}
	}

	#[test]
	#[should_panic]
	fn seed_0_fif() {
		unsafe {
			let _ = SeedInFlight::assemble(0, 0);
		}
	}

	#[test]
	fn fif_bit_packing() {
		unsafe {
			for limit in 1..=FRAMES_LIMIT {
				let seed = 0x12 + limit as u8;
				let s = SeedInFlight::assemble(seed, limit);
				for (id, fif) in s.iter().enumerate() {
					assert_eq!(fif.frame_index(), id);
					assert_eq!(fif.seed().frames_in_flight(), limit);
					assert_eq!(fif.seed().seed_u8(), seed);
				}
			}
		}
	}
}
