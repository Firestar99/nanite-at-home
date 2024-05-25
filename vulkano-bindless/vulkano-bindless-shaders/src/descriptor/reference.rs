use crate::desc_buffer::{DescStruct, MetadataCpuInterface};
use crate::descriptor::descriptor_type::DescType;
use crate::descriptor::descriptors::DescriptorsAccess;
use crate::descriptor::metadata::Metadata;
use crate::frame_in_flight::FrameInFlight;
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
	_phantom: PhantomData<(&'a (), D)>,
}

impl<'a, D: DescType + ?Sized> Copy for TransientDesc<'a, D> {}

impl<'a, D: DescType + ?Sized> Clone for TransientDesc<'a, D> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, D: DescType + ?Sized> TransientDesc<'a, D> {
	/// Create a new TransientDesc
	///
	/// # Safety
	/// id must be a valid descriptor id that stays valid for the remainder of the frame
	#[inline]
	pub const unsafe fn new(id: u32) -> Self {
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

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		Self::TransferDescStruct {
			id: self.id,
			_phantom: PhantomData {},
		}
	}

	unsafe fn read(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		// Safety: whoever wrote the TransferDescStruct must have upheld the safety contract
		unsafe { TransientDesc::new(from.id) }
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
		unsafe { TransientDesc::new(self.id) }
	}
}

#[repr(C)]
pub struct StrongDesc<'a, D: DescType + ?Sized> {
	id: u32,
	_phantom: PhantomData<(&'a (), D)>,
}

impl<'a, D: DescType + ?Sized> Copy for StrongDesc<'a, D> {}

impl<'a, D: DescType + ?Sized> Clone for StrongDesc<'a, D> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, D: DescType + ?Sized> StrongDesc<'a, D> {
	/// Create a new StrongDesc
	///
	/// # Safety
	/// id must be a valid descriptor id that is somehow ensured to stay valid for as long as this StrongDesc exists
	#[inline]
	pub const unsafe fn new(id: u32) -> Self {
		Self {
			id,
			_phantom: PhantomData {},
		}
	}

	#[inline]
	pub fn to_transient<'b>(&self, frame: FrameInFlight<'b>) -> TransientDesc<'b, D> {
		let _ = frame;
		// Safety: this StrongDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}
}

impl<'a, D: DescType + ?Sized> ValidDesc<D> for StrongDesc<'a, D> {
	#[inline]
	fn id(&self) -> u32 {
		self.id
	}
}

unsafe impl<'a, D: DescType + ?Sized> DescStruct for StrongDesc<'a, D> {
	type TransferDescStruct = TransferStrongDesc<D>;

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		_meta.visit_strong_descriptor(self);
		Self::TransferDescStruct {
			id: self.id,
			_phantom: PhantomData {},
		}
	}

	unsafe fn read(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		unsafe { StrongDesc::new(from.id) }
	}
}

#[repr(C)]
pub struct TransferStrongDesc<D: DescType + ?Sized> {
	id: u32,
	_phantom: PhantomData<&'static D>,
}

impl<D: DescType + ?Sized> Copy for TransferStrongDesc<D> {}

impl<D: DescType + ?Sized> Clone for TransferStrongDesc<D> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<D: DescType + ?Sized> Zeroable for TransferStrongDesc<D> {}

unsafe impl<D: DescType + ?Sized> AnyBitPattern for TransferStrongDesc<D> {}
