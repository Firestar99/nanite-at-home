use crate::backing::table::RcTableSlot;
use crate::descriptor::{AliveDescRef, Desc, DescContent, DescContentCpu, DescRef, DescTable, TransientAccess};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::{AnyDesc, DescriptorId, StrongDesc, TransientDesc, WeakDesc};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub struct RC<P: BindlessPlatform> {
	slot: RcTableSlot,
	_phantom: PhantomData<P>,
}

impl<P: BindlessPlatform> DescRef for RC<P> {}

impl<P: BindlessPlatform> AliveDescRef for RC<P> {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId {
		desc.r.slot.id()
	}
}

impl<P: BindlessPlatform> Clone for RC<P> {
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
			_phantom: PhantomData {},
		}
	}
}

impl<P: BindlessPlatform> Debug for RC<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("RC").field(&self.slot.id()).finish()
	}
}

impl<P: BindlessPlatform> PartialEq<Self> for RC<P> {
	fn eq(&self, other: &Self) -> bool {
		self.slot.id() == other.slot.id()
	}
}

impl<P: BindlessPlatform> Eq for RC<P> {}

impl<P: BindlessPlatform> Hash for RC<P> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.id().hash(state)
	}
}

pub type RCDesc<P, C> = Desc<RC<P>, C>;

pub trait RCDescExt<P: BindlessPlatform, C: DescContentCpu>: Sized + Hash + Eq {
	/// Create a new RCDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to.
	/// Except when Self is [`AnyRCSlot`], then this is always safe.
	unsafe fn new(slot: RcTableSlot) -> Self;

	fn rc_slot(&self) -> &RcTableSlot;

	#[inline]
	fn inner_slot(&self) -> &<C::DescTable<P> as DescTable<P>>::Slot {
		<C::DescTable<P> as DescTable<P>>::get_slot(self.rc_slot())
	}

	#[inline]
	fn id(&self) -> DescriptorId {
		self.rc_slot().id()
	}

	#[inline]
	fn to_weak(&self) -> WeakDesc<C> {
		// Safety: C does not change
		unsafe { WeakDesc::new(self.id()) }
	}

	#[inline]
	fn to_transient<'a>(&self, access: &impl TransientAccess<'a>) -> TransientDesc<'a, C> {
		// Safety: C does not change, this RCDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id(), access) }
	}

	#[inline]
	fn to_strong(&self) -> StrongDesc<C> {
		// Safety: C does not change, when calling write_cpu() this StrongDesc is visited and the slot ref inc
		unsafe { StrongDesc::new(self.id()) }
	}

	fn into_any(self) -> AnyRCDesc<P>;
}

impl<P: BindlessPlatform, C: DescContentCpu> RCDescExt<P, C> for RCDesc<P, C> {
	#[inline]
	unsafe fn new(slot: RcTableSlot) -> Self {
		unsafe {
			Desc::new_inner(RC {
				slot,
				_phantom: PhantomData {},
			})
		}
	}

	#[inline]
	fn rc_slot(&self) -> &RcTableSlot {
		&self.r.slot
	}

	#[inline]
	fn into_any(self) -> AnyRCDesc<P> {
		AnyRCDesc::new_inner(self.r)
	}
}

pub type AnyRCDesc<P> = AnyDesc<RC<P>>;

pub trait AnyRCDescExt {
	fn new(slot: RcTableSlot) -> Self;
}

impl<P: BindlessPlatform> AnyRCDescExt for AnyRCDesc<P> {
	fn new(slot: RcTableSlot) -> Self {
		AnyDesc::new_inner(RC {
			slot,
			_phantom: PhantomData {},
		})
	}
}
