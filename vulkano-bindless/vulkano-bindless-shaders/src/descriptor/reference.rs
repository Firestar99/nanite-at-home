use crate::desc_buffer::DescBuffer;
use crate::descriptor::descriptor_type::DescType;
use crate::descriptor::descriptors::DescriptorsAccess;
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

unsafe impl<D: DescType + ?Sized> bytemuck::Zeroable for TransientDesc<'static, D> {}

unsafe impl<D: DescType + ?Sized> bytemuck::AnyBitPattern for TransientDesc<'static, D> {}

unsafe impl<'a, D: DescType + ?Sized> DescBuffer for TransientDesc<'a, D> {
	type DescStatic = TransientDesc<'static, D>;

	unsafe fn to_static_desc(&self) -> Self::DescStatic {
		Self::DescStatic {
			id: self.id,
			_phantom: PhantomData {},
		}
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

unsafe impl<D: DescType + ?Sized> bytemuck::Zeroable for WeakDesc<D> {}

unsafe impl<D: DescType + ?Sized> bytemuck::AnyBitPattern for WeakDesc<D> {}

unsafe impl<D: DescType + ?Sized> DescBuffer for WeakDesc<D> {
	type DescStatic = WeakDesc<D>;

	unsafe fn to_static_desc(&self) -> Self::DescStatic {
		*self
	}
}

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
