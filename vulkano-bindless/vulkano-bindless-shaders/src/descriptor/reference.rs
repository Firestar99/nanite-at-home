use crate::desc_buffer::DescStruct;
use crate::descriptor::descriptor_type::DescType;
use crate::descriptor::descriptors::DescriptorsAccess;
use crate::descriptor::metadata::Metadata;
use bytemuck::{AnyBitPattern, Zeroable};
use core::marker::PhantomData;

pub trait ValidDesc<D: DescType + ?Sized>: Sized {
	fn id(&self) -> u32;

	#[inline]
	fn access<'a>(&'a self, descriptors: &'a impl DescriptorsAccess<D>) -> D::AccessType<'a> {
		descriptors.access(self)
	}
}

#[repr(C)]
pub struct TransientDesc<'a, D: DescType + ?Sized> {
	id: u32,
	_phantom: PhantomData<&'a D>,
}

impl<'a, D: DescType + ?Sized> Copy for TransientDesc<'a, D> {}

impl<'a, D: DescType + ?Sized> Clone for TransientDesc<'a, D> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, D: DescType + ?Sized> TransientDesc<'a, D> {
	#[inline]
	pub const fn new(id: u32) -> Self {
		Self {
			id,
			_phantom: PhantomData {},
		}
	}
}

impl<'a, D: DescType + ?Sized> ValidDesc<D> for TransientDesc<'a, D> {
	#[inline]
	fn id(&self) -> u32 {
		self.id
	}
}

unsafe impl<'a, D: DescType + ?Sized> DescStruct for TransientDesc<'a, D> {
	type TransferDescStruct = TransferTransientDesc<D>;

	unsafe fn to_transfer(self) -> Self::TransferDescStruct {
		Self::TransferDescStruct {
			id: self.id,
			_phantom: PhantomData {},
		}
	}

	unsafe fn from_transfer(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		TransientDesc::new(from.id)
	}
}

#[repr(C)]
pub struct TransferTransientDesc<D: DescType + ?Sized> {
	id: u32,
	_phantom: PhantomData<&'static D>,
}

impl<D: DescType + ?Sized> Copy for TransferTransientDesc<D> {}

impl<D: DescType + ?Sized> Clone for TransferTransientDesc<D> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<D: DescType + ?Sized> Zeroable for TransferTransientDesc<D> {}

unsafe impl<D: DescType + ?Sized> AnyBitPattern for TransferTransientDesc<D> {}

#[repr(C)]
pub struct WeakDesc<D: DescType + ?Sized> {
	id: u32,
	version: u32,
	_phantom: PhantomData<D>,
}

impl<D: DescType + ?Sized> Copy for WeakDesc<D> {}

impl<D: DescType + ?Sized> Clone for WeakDesc<D> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<D: DescType + ?Sized> Zeroable for WeakDesc<D> {}

unsafe impl<D: DescType + ?Sized> AnyBitPattern for WeakDesc<D> {}

impl<D: DescType + ?Sized> WeakDesc<D> {
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
	pub unsafe fn upgrade_unchecked<'a>(&self) -> TransientDesc<'a, D> {
		TransientDesc::new(self.id)
	}
}

// pub struct StrongRef<D: DescType + ?Sized> {
// 	id: u32,
// 	_phantom: PhantomData<D>,
// }
