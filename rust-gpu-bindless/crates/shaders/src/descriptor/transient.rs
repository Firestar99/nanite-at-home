use crate::buffer_content::{Metadata, MetadataCpuInterface};
use crate::descriptor::id::DescriptorId;
use crate::descriptor::{AliveDescRef, Desc, DescContent, DescRef, DescStructRef};
use bytemuck_derive::AnyBitPattern;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;
use core::mem;
use static_assertions::const_assert_eq;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct Transient<'a> {
	id: DescriptorId,
	_phantom: PhantomData<&'a ()>,
}
const_assert_eq!(mem::size_of::<Transient>(), 4);

impl DescRef for Transient<'_> {}

impl AliveDescRef for Transient<'_> {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId {
		desc.r.id
	}
}

impl Debug for Transient<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("Transient").field(&self.id).finish()
	}
}

pub type TransientDesc<'a, C> = Desc<Transient<'a>, C>;

impl<'a, C: DescContent> TransientDesc<'a, C> {
	/// Create a new TransientDesc
	///
	/// # Safety
	/// * The C generic must match the content that the [`DescRef`] points to.
	/// * id must be a valid descriptor id that stays valid for the remainder of the frame `'a`.
	#[inline]
	pub unsafe fn new(id: DescriptorId, access: &impl TransientAccess<'a>) -> Self {
		// We just need the lifetime from `TransientAccess`, no need to actually store the value.
		let _ = access;
		unsafe {
			Self::new_inner(Transient {
				id,
				_phantom: PhantomData {},
			})
		}
	}
}

unsafe impl<C: DescContent> DescStructRef<Desc<Self, C>> for Transient<'_> {
	type TransferDescStruct = TransferTransient;

	unsafe fn desc_write_cpu(desc: Desc<Self, C>, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		Self::TransferDescStruct { id: desc.r.id }
	}

	unsafe fn desc_read(from: Self::TransferDescStruct, _meta: Metadata) -> Desc<Self, C> {
		unsafe { TransientDesc::new(from.id, &UnsafeTransientAccess::new()) }
	}
}

#[repr(transparent)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct TransferTransient {
	id: DescriptorId,
}
const_assert_eq!(mem::size_of::<TransferTransient>(), 4);

/// Allows using this type to create `TransientDesc`, copying the lifetime `'a` of Self to `Transient`.
///
/// # Safety
/// You must ensure that any [`TransientDesc`] created from this remain valid for the duration of `'a`. This is
/// typically achieved by holding a [`BindlessFrame`] for longer than `'a`, but care must be taken when a shader is
/// recorded and submitted to the GPU operating on a [`TransientDesc`], as the lifetime will no longer be present.
pub unsafe trait TransientAccess<'a>: Sized {}

pub struct UnsafeTransientAccess<'a>(PhantomData<&'a ()>);

impl UnsafeTransientAccess<'_> {
	/// Create a UnsafeTransientAccess. Hopefully we can remove this hack at some point.
	///
	/// # Safety
	/// This allows you to construct [`TransientDesc`] with `'static` lifetime, which should never exist as they can't
	/// live on forever, so handle with care!
	pub unsafe fn new() -> Self {
		Self(PhantomData)
	}
}

unsafe impl<'a> TransientAccess<'a> for UnsafeTransientAccess<'a> {}
