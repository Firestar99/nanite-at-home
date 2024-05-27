use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::SamplerTable;
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slot::RCSlot;
use static_assertions::assert_impl_all;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::{Sampler, TransientDesc, WeakDesc};

pub struct RCDesc<D: DescTypeCpu + ?Sized> {
	any: AnyRCDesc<D::DescTable>,
}

assert_impl_all!(RCDesc<Sampler>: Send, Sync);

impl<D: DescTypeCpu + ?Sized> RCDesc<D> {
	#[inline]
	pub fn new(slot: RCSlot<<D::DescTable as DescTable>::Slot, <D::DescTable as DescTable>::RCSlotsInterface>) -> Self {
		Self {
			any: AnyRCDesc::new(slot),
		}
	}

	#[inline]
	pub fn id(&self) -> u32 {
		self.any.id()
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.any.version()
	}

	#[inline]
	pub fn to_weak(&self) -> WeakDesc<D> {
		WeakDesc::new(self.id(), self.version())
	}

	#[inline]
	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, D> {
		let _ = frame;
		// Safety: this RCDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}

	#[inline]
	pub fn to_strong(&self) -> StrongDesc<D> {
		// Safety: when calling write_cpu() this StrongDesc is visited and the slot ref inc
		unsafe { StrongDesc::new(self.id()) }
	}

	#[inline]
	pub fn into_any(self) -> AnyRCDesc<D::DescTable> {
		self.any
	}
}

impl<T: DescTypeCpu + ?Sized> Deref for RCDesc<T> {
	type Target = T::VulkanType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.any.slot)
	}
}

impl<D: DescTypeCpu + ?Sized> Clone for RCDesc<D> {
	fn clone(&self) -> Self {
		Self { any: self.any.clone() }
	}
}

impl<D: DescTypeCpu + ?Sized> Hash for RCDesc<D> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.any.hash(state)
	}
}

impl<D: DescTypeCpu + ?Sized> PartialEq<Self> for RCDesc<D> {
	fn eq(&self, other: &Self) -> bool {
		self.any == other.any
	}
}

impl<D: DescTypeCpu + ?Sized> Eq for RCDesc<D> {}

pub struct AnyRCDesc<D: DescTable> {
	slot: RCSlot<D::Slot, D::RCSlotsInterface>,
}

assert_impl_all!(AnyRCDesc<SamplerTable>: Send, Sync);

impl<D: DescTable> AnyRCDesc<D> {
	#[inline]
	pub fn new(slot: RCSlot<D::Slot, D::RCSlotsInterface>) -> Self {
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

impl<D: DescTable> Clone for AnyRCDesc<D> {
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
		}
	}
}

impl<D: DescTable> Hash for AnyRCDesc<D> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.hash(state)
	}
}

impl<D: DescTable> PartialEq<Self> for AnyRCDesc<D> {
	fn eq(&self, other: &Self) -> bool {
		self.slot == other.slot
	}
}

impl<D: DescTable> Eq for AnyRCDesc<D> {}
