use crate::backend::table::RcTableSlot;
use crate::descriptor::{AliveDescRef, Desc, DescContent, DescContentCpu, DescRef};
use crate::frame_in_flight::FrameInFlight;
use rust_gpu_bindless_shaders::descriptor::{AnyDesc, DerefDescRef, DescriptorId, StrongDesc, TransientDesc, WeakDesc};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;

#[derive(Clone)]
pub struct RC(RcTableSlot);

impl DescRef for RC {}

impl AliveDescRef for RC {
	#[inline]
	fn id<C: DescContent>(desc: &Desc<Self, C>) -> DescriptorId {
		desc.r.0.id()
	}
}

impl<C: DescContentCpu> DerefDescRef<C> for RC {
	type Target = C::VulkanType;

	fn deref(desc: &Desc<Self, C>) -> &Self::Target {
		C::deref_table(&desc.r.0)
	}
}

impl Debug for RC {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("RC").field(&self.0.id()).finish()
	}
}

impl PartialEq<Self> for RC {
	fn eq(&self, other: &Self) -> bool {
		self.0.id() == other.0.id()
	}
}

impl Eq for RC {}

impl Hash for RC {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.id().hash(state)
	}
}

pub type RCDesc<C> = Desc<RC, C>;

pub trait RCDescExt<C: DescContentCpu>: Sized + Hash + Eq + Deref<Target = C::VulkanType> {
	/// Create a new RCDesc
	///
	/// # Safety
	/// The C generic must match the content that the [`DescRef`] points to.
	/// Except when Self is [`AnyRCSlot`], then this is always safe.
	unsafe fn new(slot: RcTableSlot) -> Self;

	fn id(&self) -> DescriptorId;

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

	fn into_any(self) -> AnyRCDesc;
}

impl<C: DescContentCpu> RCDescExt<C> for RCDesc<C> {
	unsafe fn new(slot: RcTableSlot) -> Self {
		Desc::new_inner(RC(slot))
	}

	fn id(&self) -> DescriptorId {
		self.r.0.id()
	}

	#[inline]
	fn into_any(self) -> AnyRCDesc {
		AnyRCDesc::new_inner(self.r)
	}
}

pub type AnyRCDesc = AnyDesc<RC>;

pub trait AnyRCDescExt {
	fn new(slot: RcTableSlot) -> Self;
}

impl AnyRCDescExt for AnyRCDesc {
	fn new(slot: RcTableSlot) -> Self {
		AnyDesc::new_inner(RC(slot))
	}
}
