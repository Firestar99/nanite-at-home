use crate::backing::table::RcTableSlot;
use crate::descriptor::{Desc, DescContentCpu, DescContentMutCpu, RCDesc, RCDescExt};
use crate::pipeline::access_type::Undefined;
use crate::pipeline::mutable::MutBufferAccess;
use crate::platform::{BindlessPipelinePlatform, BindlessPlatform};
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{DerefDescRef, DescRef, DescriptorId, MutBuffer, MutDescRef};
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

pub trait BoxMutBufferExt<P: BindlessPipelinePlatform, T: BufferContent + ?Sized>: BoxDescExt<P, MutBuffer<T>> {
	/// Access this mutable buffer to use it for recording. Does not care for the contents of this buffer and assumes
	/// it is uninitialized.
	///
	/// # Safety
	/// Buffer must not be in use simultaneously by the Device or Host
	unsafe fn access_dont_care_unchecked<'a>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> MutBufferAccess<'a, P, T, Undefined>;
}

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized> BoxMutBufferExt<P, T> for BoxDesc<P, MutBuffer<T>> {
	unsafe fn access_dont_care_unchecked<'a>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> MutBufferAccess<'a, P, T, Undefined> {
		MutBufferAccess::new_dont_care(self, cmd)
	}
}

// TODO consider replacing this with an `into_shared` within an execution context
pub trait MutBoxDescExt<P: BindlessPlatform, C: DescContentMutCpu>: BoxDescExt<P, C> {
	fn into_shared(self) -> RCDesc<P, C::Shared>;
}

impl<P: BindlessPlatform, C: DescContentMutCpu> MutBoxDescExt<P, C> for BoxDesc<P, C> {
	#[inline]
	fn into_shared(self) -> RCDesc<P, C::Shared> {
		unsafe { RCDesc::new(self.r.slot) }
	}
}
