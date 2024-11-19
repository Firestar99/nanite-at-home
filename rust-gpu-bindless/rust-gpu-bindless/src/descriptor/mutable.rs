use crate::backend::table::RcTableSlot;
use crate::descriptor::{Desc, DescContentCpu, RCDesc, RCDescExt};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::{DerefDescRef, DescRef, DescriptorId, MutDescRef};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;

pub struct Mut<P: BindlessPlatform> {
	slot: RcTableSlot,
	_phantom: PhantomData<P>,
}

impl<P: BindlessPlatform> DescRef for Mut<P> {}

impl<P: BindlessPlatform> MutDescRef for Mut<P> {}

impl<P: BindlessPlatform, C: DescContentCpu> DerefDescRef<MutDesc<P, C>> for Mut<P> {
	type Target = C::VulkanType<P>;

	fn deref(desc: &Desc<Self, C>) -> &Self::Target {
		C::deref_table(C::get_slot(&desc.r.slot))
	}
}

impl<P: BindlessPlatform> Clone for Mut<P> {
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
			_phantom: PhantomData {},
		}
	}
}

impl<P: BindlessPlatform> Debug for Mut<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("Mut").field(&self.slot.id()).finish()
	}
}

impl<P: BindlessPlatform> PartialEq<Self> for Mut<P> {
	fn eq(&self, other: &Self) -> bool {
		self.slot.id() == other.slot.id()
	}
}

impl<P: BindlessPlatform> Eq for Mut<P> {}

impl<P: BindlessPlatform> Hash for Mut<P> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.id().hash(state)
	}
}

pub type MutDesc<P, C> = Desc<Mut<P>, C>;

pub trait MutDescExt<P: BindlessPlatform, C: DescContentCpu>:
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

	fn into_shared(self) -> RCDesc<P, C>;
}

impl<P: BindlessPlatform, C: DescContentCpu> MutDescExt<P, C> for MutDesc<P, C> {
	#[inline]
	unsafe fn new(slot: RcTableSlot) -> Self {
		Desc::new_inner(Mut {
			slot,
			_phantom: PhantomData {},
		})
	}

	#[inline]
	fn rc_slot(&self) -> &RcTableSlot {
		&self.r.slot
	}

	#[inline]
	fn into_shared(self) -> RCDesc<P, C> {
		unsafe { RCDesc::new(self.r.slot) }
	}
}
