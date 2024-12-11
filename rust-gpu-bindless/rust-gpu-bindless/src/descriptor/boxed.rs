use crate::backing::table::RcTableSlot;
use crate::descriptor::{Desc, DescContentCpu, DescContentMutCpu, RCDesc, RCDescExt};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::{
	DerefDescRef, DescRef, DescriptorId, MutDescRef, TransientAccess, TransientDesc,
};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;

pub struct Box<P: BindlessPlatform> {
	slot: RcTableSlot,
	_phantom: PhantomData<P>,
}

impl<P: BindlessPlatform> DescRef for Box<P> {}

impl<P: BindlessPlatform> MutDescRef for Box<P> {}

impl<P: BindlessPlatform, C: DescContentCpu> DerefDescRef<BoxDesc<P, C>> for Box<P> {
	type Target = C::VulkanType<P>;

	fn deref(desc: &Desc<Self, C>) -> &Self::Target {
		C::deref_table(C::get_slot(&desc.r.slot))
	}
}

impl<P: BindlessPlatform> Debug for Box<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("Mut").field(&self.slot.id()).finish()
	}
}

impl<P: BindlessPlatform> PartialEq<Self> for Box<P> {
	fn eq(&self, other: &Self) -> bool {
		self.slot.id() == other.slot.id()
	}
}

impl<P: BindlessPlatform> Eq for Box<P> {}

impl<P: BindlessPlatform> Hash for Box<P> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.id().hash(state)
	}
}

pub type BoxDesc<P, C> = Desc<Box<P>, C>;

pub trait BoxDescExt<P: BindlessPlatform, C: DescContentCpu>:
	Sized + Hash + Eq + Deref<Target = C::VulkanType<P>>
{
	/// Create a new MutDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to.
	/// Except when Self is [`AnyMutSlot`], then this is always safe.
	unsafe fn new(slot: RcTableSlot) -> Self;

	fn rc_slot(&self) -> &RcTableSlot;

	#[inline]
	fn inner_slot(&self) -> &C::Slot<P> {
		C::get_slot(self.rc_slot())
	}

	#[inline]
	fn id(&self) -> DescriptorId {
		self.rc_slot().id()
	}

	/// Create a [`TransientDesc`] pointing to the contents of this [`BoxDesc`].
	///
	/// # Safety
	/// Buffer must not be in use simultaneously by the Device
	#[inline]
	unsafe fn to_transient_unchecked<'a>(&self, access: &impl TransientAccess<'a>) -> TransientDesc<'a, C> {
		// Safety: C does not change, this BoxDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id(), access) }
	}
}

pub trait MutBoxDescExt<P: BindlessPlatform, C: DescContentMutCpu>: BoxDescExt<P, C> {
	fn into_shared(self) -> RCDesc<P, C::Shared>;
}

impl<P: BindlessPlatform, C: DescContentCpu> BoxDescExt<P, C> for BoxDesc<P, C> {
	#[inline]
	unsafe fn new(slot: RcTableSlot) -> Self {
		Desc::new_inner(Box {
			slot,
			_phantom: PhantomData {},
		})
	}

	#[inline]
	fn rc_slot(&self) -> &RcTableSlot {
		&self.r.slot
	}
}

impl<P: BindlessPlatform, C: DescContentMutCpu> MutBoxDescExt<P, C> for BoxDesc<P, C> {
	#[inline]
	fn into_shared(self) -> RCDesc<P, C::Shared> {
		unsafe { RCDesc::new(self.r.slot) }
	}
}
