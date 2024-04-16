use crate::descriptor::{DescType, ValidDesc};
use crate::frame_in_flight::FrameInFlight;
use core::marker::PhantomData;

// pub struct StrongRef<T: DType> {
// 	id: u32,
// 	_phantom: PhantomData<T>,
// }

pub struct TransientDesc<'a, D: DescType> {
	id: u32,
	_phantom: PhantomData<&'a D>,
}

impl<'a, D: DescType> TransientDesc<'a, D> {
	#[inline]
	pub const fn new(id: u32, _frame: FrameInFlight<'a>) -> Self {
		Self {
			id,
			_phantom: PhantomData {},
		}
	}
}

impl<'a, D: DescType> ValidDesc<D> for TransientDesc<'a, D> {
	#[inline]
	fn id(&self) -> u32 {
		self.id
	}
}

pub struct WeakDesc<D: DescType> {
	id: u32,
	version: u32,
	_phantom: PhantomData<D>,
}

impl<D: DescType> WeakDesc<D> {
	#[inline]
	pub const fn new(id: u32, version: u32) -> WeakDesc<D> {
		Self {
			id,
			version,
			_phantom: PhantomData {},
		}
	}

	#[inline]
	pub const fn id(&self) -> u32 {
		self.id
	}

	#[inline]
	pub const fn version(&self) -> u32 {
		self.version
	}

	/// Upgrades a WeakDesc to a TransientDesc that is valid for the current frame in flight, assuming the descriptor is still valid.
	///
	/// # Safety
	/// This unsafe variant assumes the descriptor is still alive, rather than checking whether it actually is.
	#[inline]
	pub unsafe fn upgrade_unchecked<'a>(&self, _frame: FrameInFlight<'a>) -> TransientDesc<'a, D> {
		TransientDesc::new(self.id, _frame)
	}
}
