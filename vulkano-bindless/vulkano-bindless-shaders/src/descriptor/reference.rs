use crate::desc_buffer::{DescStruct, MetadataCpuInterface};
use crate::descriptor::descriptor_content::DescContent;
use crate::descriptor::descriptors::DescriptorsAccess;
use crate::descriptor::metadata::Metadata;
use crate::frame_in_flight::FrameInFlight;
use bytemuck_derive::AnyBitPattern;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem;
use core::ops::Deref;
use static_assertions::const_assert_eq;

/// See [`Desc`].
pub trait DescRef: Sized {}

pub trait DerefDescRef<C: DescContent>: DescRef {
	type Target;

	fn deref(desc: &Desc<Self, C>) -> &Self::Target;
}

/// A generic Descriptor.
///
/// The T generic describes the type of descriptor this is. Think of it as representing the type of smart pointer you
/// want to use, implemented by types similar to [`Rc`] or [`Arc`]. But it may also control when you'll have access to
/// it, as similar to a [`Weak`] pointer the backing object could have deallocated.
///
/// The C generic describes the Contents that this pointer is pointing to. This may plainly be a typed [`Buffer<R>`],
/// but could also be a `UniformConstant` like an [`Image`], [`Sampler`] or others.
#[repr(C)]
pub struct Desc<R: DescRef, C: DescContent> {
	pub r: R,
	_phantom: PhantomData<&'static C>,
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

impl<R: DerefDescRef<C>, C: DescContent> Deref for Desc<R, C> {
	type Target = R::Target;

	fn deref(&self) -> &Self::Target {
		R::deref(self)
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

unsafe impl<R: DescRef, C: DescContent> DescStruct for Desc<R, C>
where
	R: DescStruct,
{
	type TransferDescStruct = R::TransferDescStruct;

	#[inline]
	unsafe fn write_cpu(self, meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		// Safety: delegated
		unsafe { self.r.write_cpu(meta) }
	}

	#[inline]
	unsafe fn read(from: Self::TransferDescStruct, meta: Metadata) -> Self {
		// Safety: delegated
		unsafe { Self::new_inner(R::read(from, meta)) }
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

unsafe impl<R: DescRef> DescStruct for AnyDesc<R>
where
	R: DescStruct,
{
	type TransferDescStruct = R::TransferDescStruct;

	#[inline]
	unsafe fn write_cpu(self, meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		// Safety: delegated
		unsafe { self.r.write_cpu(meta) }
	}

	#[inline]
	unsafe fn read(from: Self::TransferDescStruct, meta: Metadata) -> Self {
		// Safety: delegated
		unsafe { Self::new_inner(R::read(from, meta)) }
	}
}

/// A [`DescRef`] that somehow ensures the content it's pointing to is always alive, allowing it to be accessed.
pub trait AliveDescRef: DescRef {
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> u32;
}

impl<R: AliveDescRef, C: DescContent> Desc<R, C> {
	pub fn id(&self) -> u32 {
		R::id(self)
	}

	#[inline]
	pub fn access<'a>(&self, descriptors: &'a impl DescriptorsAccess<C>) -> C::AccessType<'a> {
		descriptors.access(self)
	}
}

// transient
#[derive(Copy, Clone)]
pub struct Transient<'a> {
	id: u32,
	_phantom: PhantomData<&'a ()>,
}
const_assert_eq!(mem::size_of::<Transient>(), 4);

impl<'a> DescRef for Transient<'a> {}

impl<'a> AliveDescRef for Transient<'a> {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> u32 {
		desc.r.id
	}
}

pub type TransientDesc<'a, C> = Desc<Transient<'a>, C>;

impl<'a, C: DescContent> TransientDesc<'a, C> {
	/// Create a new TransientDesc
	///
	/// # Safety
	/// * The C generic must match the content that the [`DescRef`] points to.
	/// * id must be a valid descriptor id that stays valid for the remainder of the frame.
	#[inline]
	pub const unsafe fn new(id: u32) -> Self {
		unsafe {
			Self::new_inner(Transient {
				id,
				_phantom: PhantomData {},
			})
		}
	}
}

unsafe impl<'a> DescStruct for Transient<'a> {
	type TransferDescStruct = TransferTransient;

	#[inline]
	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		Self::TransferDescStruct { id: self.id }
	}

	#[inline]
	unsafe fn read(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		Transient {
			id: from.id,
			_phantom: PhantomData {},
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct TransferTransient {
	id: u32,
}

// weak
#[derive(Copy, Clone, AnyBitPattern)]
pub struct Weak {
	id: u32,
	version: u32,
}
const_assert_eq!(mem::size_of::<Weak>(), 8);

impl DescRef for Weak {}

pub type WeakDesc<C> = Desc<Weak, C>;

impl<C: DescContent> WeakDesc<C> {
	/// Creates a new WeakDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	pub const unsafe fn new(id: u32, version: u32) -> WeakDesc<C> {
		unsafe { Self::new_inner(Weak { id, version }) }
	}

	#[inline]
	pub const fn id(&self) -> u32 {
		self.r.id
	}

	#[inline]
	pub const fn version(&self) -> u32 {
		self.r.version
	}

	/// Upgrades a WeakDesc to a TransientDesc that is valid for the current frame in flight, assuming the descriptor
	/// pointed to is still valid.
	///
	/// # Safety
	/// This unsafe variant assumes the descriptor is still alive, rather than checking whether it actually is.
	#[inline]
	pub unsafe fn upgrade_unchecked<'a>(&self) -> TransientDesc<'a, C> {
		unsafe { TransientDesc::new(self.r.id) }
	}
}

// strong
#[derive(Copy, Clone)]
pub struct Strong {
	id: u32,
	/// internal value only used on the CPU to validate that slot wasn't reused
	version: u32,
}
const_assert_eq!(mem::size_of::<Strong>(), 8);

impl DescRef for Strong {}

impl AliveDescRef for Strong {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> u32 {
		desc.r.id
	}
}

pub type StrongDesc<C> = Desc<Strong, C>;

impl<C: DescContent> StrongDesc<C> {
	/// Create a new StrongDesc
	///
	/// # Safety
	/// id must be a valid descriptor id that is somehow ensured to stay valid for as long as this StrongDesc exists
	#[inline]
	pub const unsafe fn new(id: u32, version: u32) -> Self {
		unsafe { Self::new_inner(Strong { id, version }) }
	}

	/// Get the version
	///
	/// # Safety
	/// only available on the cpu
	#[cfg(not(target_arch = "spirv"))]
	pub unsafe fn version_cpu(&self) -> u32 {
		self.r.version
	}

	#[inline]
	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, C> {
		let _ = frame;
		// Safety: this StrongDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}
}

unsafe impl<C: DescContent> DescStruct for StrongDesc<C> {
	type TransferDescStruct = TransferStrong;

	#[inline]
	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::TransferDescStruct {
		_meta.visit_strong_descriptor(self);
		Self::TransferDescStruct { id: self.r.id }
	}

	#[inline]
	unsafe fn read(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		unsafe { StrongDesc::new(from.id, 0) }
	}
}

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct TransferStrong {
	id: u32,
}
