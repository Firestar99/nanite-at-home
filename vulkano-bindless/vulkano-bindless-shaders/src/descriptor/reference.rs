use crate::desc_buffer::{DescStruct, MetadataCpuInterface};
use crate::descriptor::descriptor_content::DescContent;
use crate::descriptor::descriptors::DescriptorsAccess;
use crate::descriptor::metadata::Metadata;
use crate::frame_in_flight::FrameInFlight;
use bytemuck::{AnyBitPattern, Zeroable};
use core::marker::PhantomData;

pub trait ValidDesc<C: DescContent + ?Sized>: Sized {
	fn id(&self) -> u32;

	#[inline]
	fn access<'a>(&self, descriptors: &'a impl DescriptorsAccess<C>) -> C::AccessType<'a> {
		descriptors.access(self)
	}
}

#[repr(C)]
pub struct TransientDesc<'a, C: DescContent + ?Sized> {
	id: u32,
	_phantom: PhantomData<(&'a (), C)>,
}

impl<'a, C: DescContent + ?Sized> Copy for TransientDesc<'a, C> {}

impl<'a, C: DescContent + ?Sized> Clone for TransientDesc<'a, C> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, C: DescContent + ?Sized> TransientDesc<'a, C> {
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

impl<'a, C: DescContent + ?Sized> ValidDesc<C> for TransientDesc<'a, C> {
	#[inline]
	fn id(&self) -> u32 {
		self.id
	}
}

unsafe impl<'a, C: DescContent + ?Sized> DescStruct for TransientDesc<'a, C> {
	type TransferDescStruct = TransferTransientDesc<C>;

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
pub struct TransferTransientDesc<C: DescContent + ?Sized> {
	id: u32,
	_phantom: PhantomData<&'static C>,
}

impl<C: DescContent + ?Sized> Copy for TransferTransientDesc<C> {}

impl<C: DescContent + ?Sized> Clone for TransferTransientDesc<C> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<C: DescContent + ?Sized> Zeroable for TransferTransientDesc<C> {}

unsafe impl<C: DescContent + ?Sized> AnyBitPattern for TransferTransientDesc<C> {}

#[repr(C)]
pub struct WeakDesc<C: DescContent + ?Sized> {
	id: u32,
	version: u32,
	_phantom: PhantomData<C>,
}

impl<C: DescContent + ?Sized> Copy for WeakDesc<C> {}

impl<C: DescContent + ?Sized> Clone for WeakDesc<C> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<C: DescContent + ?Sized> Zeroable for WeakDesc<C> {}

unsafe impl<C: DescContent + ?Sized> AnyBitPattern for WeakDesc<C> {}

impl<C: DescContent + ?Sized> WeakDesc<C> {
	#[inline]
	pub const fn new(id: u32, version: u32) -> WeakDesc<C> {
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
	pub unsafe fn upgrade_unchecked<'a>(&self) -> TransientDesc<'a, C> {
		unsafe { TransientDesc::new(self.id) }
	}
}

#[repr(C)]
pub struct StrongDesc<C: DescContent + ?Sized> {
	id: u32,
	/// internal value only used on the CPU to validate that slot wasn't reused
	version: u32,
	_phantom: PhantomData<C>,
}

impl<C: DescContent + ?Sized> Copy for StrongDesc<C> {}

impl<C: DescContent + ?Sized> Clone for StrongDesc<C> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<C: DescContent + ?Sized> StrongDesc<C> {
	/// Create a new StrongDesc
	///
	/// # Safety
	/// id must be a valid descriptor id that is somehow ensured to stay valid for as long as this StrongDesc exists
	#[inline]
	pub const unsafe fn new(id: u32, version: u32) -> Self {
		Self {
			id,
			version,
			_phantom: PhantomData {},
		}
	}

	/// Get the version
	///
	/// # Safety
	/// only available on the cpu
	#[cfg(not(target_arch = "spirv"))]
	pub unsafe fn version_cpu(&self) -> u32 {
		self.version
	}

	#[inline]
	pub fn to_transient<'b>(&self, frame: FrameInFlight<'b>) -> TransientDesc<'b, C> {
		let _ = frame;
		// Safety: this StrongDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}
}

impl<C: DescContent + ?Sized> ValidDesc<C> for StrongDesc<C> {
	#[inline]
	fn id(&self) -> u32 {
		self.id
	}
}

unsafe impl<C: DescContent + ?Sized> DescStruct for StrongDesc<C> {
	type TransferDescStruct = TransferStrongDesc<C>;

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		_meta.visit_strong_descriptor(self);
		Self::TransferDescStruct {
			id: self.id,
			_phantom: PhantomData {},
		}
	}

	unsafe fn read(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		unsafe { StrongDesc::new(from.id, 0) }
	}
}

#[repr(C)]
pub struct TransferStrongDesc<C: DescContent + ?Sized> {
	id: u32,
	_phantom: PhantomData<&'static C>,
}

impl<C: DescContent + ?Sized> Copy for TransferStrongDesc<C> {}

impl<C: DescContent + ?Sized> Clone for TransferStrongDesc<C> {
	fn clone(&self) -> Self {
		*self
	}
}

unsafe impl<C: DescContent + ?Sized> Zeroable for TransferStrongDesc<C> {}

unsafe impl<C: DescContent + ?Sized> AnyBitPattern for TransferStrongDesc<C> {}
