use crate::descriptor::descriptor_content::{DescContentCpu, DescTable, DescTableEnum, DescTableEnumType};
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slot::RCSlot;
use static_assertions::assert_impl_all;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::{AnyDesc, DerefDescRef, DescRef, DescriptorId, StrongDesc};
use vulkano_bindless_shaders::descriptor::{Desc, Sampler, TransientDesc, WeakDesc};

/// Trait defining all common impl between `Desc<RC, C>` and `RCDesc<C>`
pub trait RCDescExt<C: DescContentCpu>:
	Sized + Hash + Eq + From<RCDesc<C>> + From<Desc<RC, C>> + Deref<Target = C::VulkanType>
{
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	/// Except when Self is [`AnyRCSlot`], then this is always safe.
	unsafe fn from_inner(inner: RCInner<C::DescTable>) -> Self;

	fn inner(&self) -> &RCInner<C::DescTable>;

	fn into_inner(self) -> RCInner<C::DescTable>;

	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to.
	/// Except when Self is [`AnyRCSlot`], then this is always safe.
	#[inline]
	unsafe fn new(
		slot: RCSlot<<C::DescTable as DescTable>::Slot, <C::DescTable as DescTable>::RCSlotsInterface>,
	) -> Self {
		unsafe { Self::from_inner(RCInner::new(slot)) }
	}

	#[inline]
	fn id(&self) -> DescriptorId {
		self.inner().id()
	}

	#[inline]
	fn to_weak(&self) -> WeakDesc<C> {
		// Safety: C does not change
		unsafe { WeakDesc::new(self.id()) }
	}

	#[inline]
	fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, C> {
		// Safety: C does not change, this RCDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id(), frame) }
	}

	#[inline]
	fn to_strong(&self) -> StrongDesc<C> {
		// Safety: C does not change, when calling write_cpu() this StrongDesc is visited and the slot ref inc
		unsafe { StrongDesc::new(self.id()) }
	}

	#[inline]
	fn into_any(self) -> AnyRCDesc {
		AnyRCDesc::from_inner(self.into_inner())
	}
}

impl<C: DescContentCpu> DerefDescRef<C> for RC {
	type Target = C::VulkanType;

	fn deref(desc: &Desc<Self, C>) -> &Self::Target {
		C::deref_table(&desc.inner().slot)
	}
}

impl<C: DescContentCpu> From<RCDesc<C>> for Desc<RC, C> {
	#[inline]
	fn from(desc: RCDesc<C>) -> Self {
		// Safety: C does not change
		unsafe { Desc::from_inner(desc.inner) }
	}
}

const RC_CONTENT_MISMATCH: &str = "Content's table type did not match table type of RCInner!";

impl<C: DescContentCpu> RCDescExt<C> for Desc<RC, C> {
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	unsafe fn from_inner(inner: RCInner<C::DescTable>) -> Self {
		unsafe { Self::new_inner(RC::from_inner(inner)) }
	}

	#[inline]
	fn inner(&self) -> &RCInner<C::DescTable> {
		self.r.try_deref::<C::DescTable>().expect(RC_CONTENT_MISMATCH)
	}

	fn into_inner(self) -> RCInner<C::DescTable> {
		self.r.try_into::<C::DescTable>().ok().expect(RC_CONTENT_MISMATCH)
	}
}

/// See [`RC`].
///
/// This is basically a `Desc<RC, C>`, but implemented with a custom type to remove the unnecessary enum tag of [`RC`]
/// and instead uses [`RCInner`] directly.
pub struct RCDesc<C: DescContentCpu> {
	inner: RCInner<C::DescTable>,
}
assert_impl_all!(RCDesc<Sampler>: Send, Sync);

impl<C: DescContentCpu> From<Desc<RC, C>> for RCDesc<C> {
	#[inline]
	fn from(desc: Desc<RC, C>) -> Self {
		// Safety: C does not change
		unsafe { Self::from_inner(desc.into_inner()) }
	}
}

impl<C: DescContentCpu> RCDescExt<C> for RCDesc<C> {
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	unsafe fn from_inner(inner: RCInner<C::DescTable>) -> Self {
		Self { inner }
	}

	#[inline]
	fn inner(&self) -> &RCInner<C::DescTable> {
		&self.inner
	}

	fn into_inner(self) -> RCInner<C::DescTable> {
		self.inner
	}
}

impl<C: DescContentCpu> Deref for RCDesc<C> {
	type Target = C::VulkanType;

	#[inline]
	fn deref(&self) -> &Self::Target {
		C::deref_table(&self.inner.slot)
	}
}

impl<C: DescContentCpu> Clone for RCDesc<C> {
	#[inline]
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

impl<C: DescContentCpu> Hash for RCDesc<C> {
	#[inline]
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.inner.hash(state)
	}
}

impl<C: DescContentCpu> PartialEq<Self> for RCDesc<C> {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<C: DescContentCpu> Eq for RCDesc<C> {}

/// AnyDesc<RC> cannot use the enum optimization, so just the usual `AnyDesc<RC>`
pub type AnyRCDesc = AnyDesc<RC>;

pub trait AnyRCDescExt: Sized + Hash + Eq {
	fn from_inner<T: DescTable>(inner: RCInner<T>) -> Self;

	#[inline]
	fn new<T: DescTable>(slot: RCSlot<<T as DescTable>::Slot, <T as DescTable>::RCSlotsInterface>) -> Self {
		Self::from_inner(RCInner::<T>::new(slot))
	}

	fn id(&self) -> u32;

	fn version(&self) -> u32;
}

impl AnyRCDescExt for AnyRCDesc {
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to
	#[inline]
	fn from_inner<T: DescTable>(inner: RCInner<T>) -> Self {
		AnyRCDesc::new_inner(RC::from_inner(inner))
	}

	#[inline]
	fn id(&self) -> u32 {
		self.r.id()
	}

	#[inline]
	fn version(&self) -> u32 {
		self.r.version()
	}
}

/// RCInner reference counts a slot within a [`DescTable`] specified by the generic `T`. See [`RC`].
pub struct RCInner<T: DescTable> {
	slot: RCSlot<T::Slot, T::RCSlotsInterface>,
}

impl<T: DescTable> RCInner<T> {
	#[inline]
	pub fn new(slot: RCSlot<T::Slot, T::RCSlotsInterface>) -> Self {
		Self { slot }
	}

	#[inline]
	pub fn id(&self) -> DescriptorId {
		// we guarantee 32 bits is enough when constructing the resource tables
		todo!()
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.slot.version()
	}
}

impl<T: DescTable> Clone for RCInner<T> {
	#[inline]
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
		}
	}
}

impl<T: DescTable> Hash for RCInner<T> {
	#[inline]
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.hash(state)
	}
}

impl<T: DescTable> PartialEq<Self> for RCInner<T> {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.slot == other.slot
	}
}

impl<T: DescTable> Eq for RCInner<T> {}

/// A reference counted [`DescRef`] that only works on the CPU. We do not want reference counting on the GPU, as it's
/// very inefficient with the memory bandwidth and atomic operations required. When a descriptor should be sent to the
/// GPU, it first has to be converted to another GPU-compatible reference type using [`RCDesc::to_transient`],
/// [`RCDesc::to_strong`] or [`RCDesc::to_weak`], depending on the lifetime requirements for the descriptor.
///
/// Impl Note: RC is like [`RCInner`] but doesn't take the table type as a generic, instead it's an enum choosing
/// between the different tables.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct RC(DescTableEnum<RCDescTableEnumType>);
assert_impl_all!(RC: Send, Sync);

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct RCDescTableEnumType;

impl DescTableEnumType for RCDescTableEnumType {
	type Type<T: DescTable> = RCInner<T>;
}

impl DescRef for RC {}

impl AnyRCDescExt for RC {
	#[inline]
	fn from_inner<T: DescTable>(inner: RCInner<T>) -> Self {
		Self(DescTableEnum::new(inner))
	}

	#[inline]
	fn id(&self) -> u32 {
		todo!()
	}

	#[inline]
	fn version(&self) -> u32 {
		match &self.0 {
			DescTableEnum::Buffer(b) => b.version(),
			DescTableEnum::Image(i) => i.version(),
			DescTableEnum::Sampler(s) => s.version(),
		}
	}
}

impl RC {
	#[inline]
	pub fn try_deref<T: DescTable>(&self) -> Option<&RCInner<T>> {
		self.0.try_deref()
	}

	#[inline]
	pub fn try_into<T: DescTable>(self) -> Result<RCInner<T>, DescTableEnum<RCDescTableEnumType>> {
		self.0.try_into()
	}
}
