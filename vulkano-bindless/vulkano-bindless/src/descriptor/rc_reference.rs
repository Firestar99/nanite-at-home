use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::{BufferTable, ImageTable, SamplerTable};
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slot::RCSlot;
use static_assertions::assert_impl_all;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::reference::{DescRef, StrongDesc};
use vulkano_bindless_shaders::descriptor::{Desc, Sampler, TransientDesc, WeakDesc};

/// See [`RC`].
///
/// This is basically a `Desc<RC, C>`, but implemented with a custom type to remove the unnecessary enum tag of [`RC`]
/// and instead uses [`RCInner`] directly.
pub struct RCDesc<C: DescContentCpu> {
	inner: RCInner<C::DescTable>,
}
assert_impl_all!(RCDesc<Sampler>: Send, Sync);

impl<C: DescContentCpu> RCDesc<C> {
	#[inline]
	pub fn new(slot: RCSlot<<C::DescTable as DescTable>::Slot, <C::DescTable as DescTable>::RCSlotsInterface>) -> Self {
		Self::from_inner(RCInner::new(slot))
	}

	#[inline]
	pub fn from_inner(inner: RCInner<C::DescTable>) -> Self {
		Self { inner }
	}

	#[inline]
	pub fn id(&self) -> u32 {
		self.inner.id()
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.inner.version()
	}

	#[inline]
	pub fn to_weak(&self) -> WeakDesc<C> {
		// Safety: C does not change
		unsafe { WeakDesc::new(self.id(), self.version()) }
	}

	#[inline]
	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, C> {
		let _ = frame;
		// Safety: C does not change, this RCDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}

	#[inline]
	pub fn to_strong(&self) -> StrongDesc<C> {
		// Safety: C does not change, when calling write_cpu() this StrongDesc is visited and the slot ref inc
		unsafe { StrongDesc::new(self.id(), self.version()) }
	}

	#[inline]
	pub fn into_inner(self) -> RCInner<C::DescTable> {
		self.inner
	}
}

impl<C: DescContentCpu> RCDesc<C>
where
	RC: RCNewExt<<C as DescContentCpu>::DescTable>,
{
	pub fn into_desc(self) -> Desc<RC, C> {
		// Safety: C does not change
		unsafe { Desc::new_inner(RC::from_inner(self.inner)) }
	}

	pub fn from_desc(desc: Desc<RC, C>) -> Self {
		Self::from_inner(RCNewExt::to_inner(desc.r).expect("Content's table type did not match table type of RCInner!"))
	}
}

impl<T: DescContentCpu> Deref for RCDesc<T> {
	type Target = T::VulkanType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.inner.slot)
	}
}

impl<C: DescContentCpu> Clone for RCDesc<C> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

impl<C: DescContentCpu> Hash for RCDesc<C> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.inner.hash(state)
	}
}

impl<C: DescContentCpu> PartialEq<Self> for RCDesc<C> {
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<C: DescContentCpu> Eq for RCDesc<C> {}

/// RCInner reference counts a slot within a [`DescTable`] specified by the generic `T`. See [`RC`].
pub struct RCInner<T: DescTable> {
	slot: RCSlot<T::Slot, T::RCSlotsInterface>,
}
assert_impl_all!(RCInner<SamplerTable>: Send, Sync);

impl<T: DescTable> RCInner<T> {
	#[inline]
	pub fn new(slot: RCSlot<T::Slot, T::RCSlotsInterface>) -> Self {
		Self { slot }
	}

	#[inline]
	pub fn id(&self) -> u32 {
		// we guarantee 32 bits is enough when constructing the resource tables
		*self.slot.id() as u32
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.slot.version()
	}
}

impl<T: DescTable> Clone for RCInner<T> {
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
		}
	}
}

impl<T: DescTable> Hash for RCInner<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.hash(state)
	}
}

impl<T: DescTable> PartialEq<Self> for RCInner<T> {
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
pub enum RC {
	Buffer(RCInner<BufferTable>),
	Image(RCInner<ImageTable>),
	Sampler(RCInner<SamplerTable>),
}
assert_impl_all!(RC: Send, Sync);

impl DescRef for RC {}

impl RC {
	#[inline]
	pub fn id(&self) -> u32 {
		match self {
			RC::Buffer(b) => b.id(),
			RC::Image(i) => i.id(),
			RC::Sampler(s) => s.id(),
		}
	}

	#[inline]
	pub fn version(&self) -> u32 {
		match self {
			RC::Buffer(b) => b.version(),
			RC::Image(i) => i.version(),
			RC::Sampler(s) => s.version(),
		}
	}
}

pub trait RCNewExt<T: DescTable>: Sized {
	#[inline]
	fn new(slot: RCSlot<T::Slot, T::RCSlotsInterface>) -> Self {
		Self::from_inner(RCInner::new(slot))
	}

	fn from_inner(inner: RCInner<T>) -> Self;

	fn to_inner(self) -> Option<RCInner<T>>;
}

impl RCNewExt<BufferTable> for RC {
	#[inline]
	fn from_inner(inner: RCInner<BufferTable>) -> Self {
		Self::Buffer(inner)
	}

	#[inline]
	fn to_inner(self) -> Option<RCInner<BufferTable>> {
		if let Self::Buffer(b) = self {
			Some(b)
		} else {
			None
		}
	}
}

impl RCNewExt<ImageTable> for RC {
	#[inline]
	fn from_inner(inner: RCInner<ImageTable>) -> Self {
		Self::Image(inner)
	}

	#[inline]
	fn to_inner(self) -> Option<RCInner<ImageTable>> {
		if let Self::Image(b) = self {
			Some(b)
		} else {
			None
		}
	}
}

impl RCNewExt<SamplerTable> for RC {
	#[inline]
	fn from_inner(inner: RCInner<SamplerTable>) -> Self {
		Self::Sampler(inner)
	}

	#[inline]
	fn to_inner(self) -> Option<RCInner<SamplerTable>> {
		if let Self::Sampler(b) = self {
			Some(b)
		} else {
			None
		}
	}
}
