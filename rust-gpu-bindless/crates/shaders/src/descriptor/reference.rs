use crate::buffer_content::{BufferStruct, Metadata, MetadataCpuInterface};
use crate::descriptor::descriptor_content::DescContent;
use crate::descriptor::descriptors::DescriptorAccess;
use crate::descriptor::id::DescriptorId;
use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

/// A shared read-only [`Desc`].
pub trait DescRef: Sized + Send + Sync {}

/// A mutable [`Desc`].
pub trait MutDescRef: DescRef {}

/// A generic Descriptor.
///
/// The T generic describes the type of descriptor this is. Think of it as representing the type of smart pointer you
/// want to use, implemented by types similar to [`Rc`] or [`Arc`]. But it may also control when you'll have access to
/// it, similar to a [`Weak`] pointer the backing object could have deallocated. And what kind of access you have,
/// whether this is a read-only shared resource or a mutable resource.
///
/// The C generic describes the Contents that this pointer is pointing to. This may plainly be a typed [`Buffer<R>`],
/// but could also be a `UniformConstant` like an [`Image`], [`Sampler`] or others.
#[repr(C)]
pub struct Desc<R: DescRef, C: DescContent> {
	pub r: R,
	_phantom: PhantomData<C>,
}

impl<R: DescRef, C: DescContent> Desc<R, C> {
	/// Creates a new Desc from some [`DescRef`]
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	pub const unsafe fn new_inner(r: R) -> Self {
		Self {
			r,
			_phantom: PhantomData,
		}
	}

	#[inline]
	pub fn into_any(self) -> AnyDesc<R> {
		AnyDesc::new_inner(self.r)
	}
}

impl<R: DescRef + Copy, C: DescContent> Copy for Desc<R, C> {}

impl<R: DescRef + Clone, C: DescContent> Clone for Desc<R, C> {
	#[inline]
	fn clone(&self) -> Self {
		Self {
			r: self.r.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<R: DescRef + Debug, C: DescContent> Debug for Desc<R, C> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("Desc").field(&self.r).finish()
	}
}

impl<R: DescRef + Hash, C: DescContent> Hash for Desc<R, C> {
	#[inline]
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.r.hash(state)
	}
}

impl<R: DescRef + PartialEq, C: DescContent> PartialEq for Desc<R, C> {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.r == other.r
	}
}

impl<R: DescRef + Eq, C: DescContent> Eq for Desc<R, C> {}

/// works just like [`BufferStruct`] but on [`Desc`] instead of Self
///
/// # Safety
/// see [`BufferStruct`]
#[allow(clippy::missing_safety_doc)]
pub unsafe trait DescStructRef<D>: Copy {
	type TransferDescStruct: bytemuck::AnyBitPattern + Send + Sync;

	unsafe fn desc_write_cpu(desc: D, meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct;

	unsafe fn desc_read(from: Self::TransferDescStruct, meta: Metadata) -> D;
}

unsafe impl<R: DescRef + DescStructRef<Self>, C: DescContent> BufferStruct for Desc<R, C> {
	type Transfer = R::TransferDescStruct;

	#[inline]
	unsafe fn write_cpu(self, meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		// Safety: delegated
		unsafe { R::desc_write_cpu(self, meta) }
	}

	#[inline]
	unsafe fn read(from: Self::Transfer, meta: Metadata) -> Self {
		// Safety: delegated
		unsafe { R::desc_read(from, meta) }
	}
}

/// AnyDesc is a [`Desc`] that does not care for the contents the reference is pointing to, only for the reference
/// existing. This is particularly useful with RC (reference counted), to keep content alive without having to know what
/// it is. Create using [`Desc::into_any`]
#[repr(C)]
pub struct AnyDesc<R: DescRef> {
	pub r: R,
}

impl<R: DescRef> AnyDesc<R> {
	/// Creates a new AnyDesc from some [`DescRef`]
	#[inline]
	pub const fn new_inner(r: R) -> Self {
		Self { r }
	}
}

impl<R: DescRef + Copy> Copy for AnyDesc<R> {}

impl<R: DescRef + Clone> Clone for AnyDesc<R> {
	#[inline]
	fn clone(&self) -> Self {
		Self { r: self.r.clone() }
	}
}

impl<R: DescRef + Hash> Hash for AnyDesc<R> {
	#[inline]
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.r.hash(state)
	}
}

impl<R: DescRef + PartialEq> PartialEq for AnyDesc<R> {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.r == other.r
	}
}

impl<R: DescRef + Eq> Eq for AnyDesc<R> {}

/// A [`DescRef`] that somehow ensures the content it's pointing to is always alive, allowing it to be accessed.
pub trait AliveDescRef: DescRef {
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId;
}

impl<R: AliveDescRef, C: DescContent> Desc<R, C> {
	pub fn id(&self) -> DescriptorId {
		R::id(self)
	}

	#[inline]
	pub fn access<'a, AT: DescriptorAccess<'a, C>>(&self, descriptors: AT) -> AT::AccessType {
		descriptors.access(self)
	}
}
