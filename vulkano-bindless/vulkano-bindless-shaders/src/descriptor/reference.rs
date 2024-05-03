use crate::descriptor::descriptor_type::DescType;
use crate::descriptor::descriptors::Descriptors;
use crate::frame_in_flight::FrameInFlight;
use bytemuck_derive::AnyBitPattern;
use core::marker::PhantomData;

pub trait ValidDesc<D: DescType> {
	fn id(&self) -> u32;

	fn access<'a>(&'a self, descriptors: &'a Descriptors<'a>) -> D::AccessType<'a> {
		D::access(descriptors, self.id())
	}
}

#[repr(C)]
pub struct TransientDesc<'a, D: DescType> {
	id: u32,
	_phantom: PhantomData<&'a D>,
}

impl<'a, D: DescType> Copy for TransientDesc<'a, D> {}

impl<'a, D: DescType> Clone for TransientDesc<'a, D> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<D: DescType> bytemuck::Zeroable for TransientDesc<'static, D> {}

unsafe impl<D: DescType> bytemuck::AnyBitPattern for TransientDesc<'static, D> {}

impl<'a, D: DescType> TransientDesc<'a, D> {
	#[inline]
	pub const fn new(id: u32, _frame: FrameInFlight<'a>) -> Self {
		Self {
			id,
			_phantom: PhantomData {},
		}
	}

	#[inline]
	pub unsafe fn to_static(&self) -> TransientDesc<'static, D> {
		TransientDesc {
			id: self.id,
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

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
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

// pub struct StrongRef<T: DType> {
// 	id: u32,
// 	_phantom: PhantomData<T>,
// }
