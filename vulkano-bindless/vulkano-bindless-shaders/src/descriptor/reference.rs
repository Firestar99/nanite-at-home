use std::marker::PhantomData;

use crate::descriptor::DescType;
use crate::frame_in_flight::FrameInFlight;

// pub struct StrongRef<T: DType> {
// 	id: u32,
// 	_phantom: PhantomData<T>,
// }

pub struct TransientDesc<'a, T: DescType> {
	id: u32,
	_phantom: PhantomData<&'a T>,
}

impl<'a, T: DescType> TransientDesc<'a, T> {
	pub const fn new(id: u32, _frame: FrameInFlight<'a>) -> Self {
		Self {
			id,
			_phantom: PhantomData {},
		}
	}
}

pub struct WeakDesc<T: DescType> {
	id: u32,
	version: u32,
	_phantom: PhantomData<T>,
}

impl<T: DescType> WeakDesc<T> {
	pub const fn new(id: u32, version: u32) -> WeakDesc<T> {
		Self {
			id,
			version,
			_phantom: PhantomData {},
		}
	}

	/// Upgrades a WeakDesc to a TransientDesc that is valid for the current frame in flight, assuming the descriptor is still valid. 
	///
	/// # Safety
	/// This unsafe variant assumes the descriptor is still alive, rather than checking whether it actually is.
	pub unsafe fn upgrade_unchecked<'a>(&self, _frame: FrameInFlight<'a>) -> TransientDesc<'a, T> {
		TransientDesc::new(self.id, _frame)
	}
}
