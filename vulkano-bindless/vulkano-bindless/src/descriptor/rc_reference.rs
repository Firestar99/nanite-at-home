use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::SamplerTable;
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slot::RCSlot;
use static_assertions::assert_impl_all;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::reference::{DescRef, StrongDesc};
use vulkano_bindless_shaders::descriptor::{Sampler, TransientDesc, WeakDesc};

pub struct RCDesc<C: DescContentCpu + ?Sized> {
	any: RC<C::DescTable>,
}

assert_impl_all!(RCDesc<Sampler>: Send, Sync);

impl<C: DescContentCpu + ?Sized> RCDesc<C> {
	#[inline]
	pub fn new(slot: RCSlot<<C::DescTable as DescTable>::Slot, <C::DescTable as DescTable>::RCSlotsInterface>) -> Self {
		Self { any: RC::new(slot) }
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
	pub fn to_weak(&self) -> WeakDesc<C> {
		WeakDesc::new(self.id(), self.version())
	}

	#[inline]
	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, C> {
		let _ = frame;
		// Safety: this RCDesc existing ensures the descriptor will stay alive for this frame
		unsafe { TransientDesc::new(self.id()) }
	}

	#[inline]
	pub fn to_strong(&self) -> StrongDesc<C> {
		// Safety: when calling write_cpu() this StrongDesc is visited and the slot ref inc
		unsafe { StrongDesc::new(self.id(), self.version()) }
	}

	#[inline]
	pub fn into_any(self) -> RC<C::DescTable> {
		self.any
	}
}

impl<T: DescContentCpu + ?Sized> Deref for RCDesc<T> {
	type Target = T::VulkanType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.any.slot)
	}
}

impl<C: DescContentCpu + ?Sized> Clone for RCDesc<C> {
	fn clone(&self) -> Self {
		Self { any: self.any.clone() }
	}
}

impl<C: DescContentCpu + ?Sized> Hash for RCDesc<C> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.any.hash(state)
	}
}

impl<C: DescContentCpu + ?Sized> PartialEq<Self> for RCDesc<C> {
	fn eq(&self, other: &Self) -> bool {
		self.any == other.any
	}
}

impl<C: DescContentCpu + ?Sized> Eq for RCDesc<C> {}

pub struct RC<C: DescTable> {
	slot: RCSlot<C::Slot, C::RCSlotsInterface>,
}

assert_impl_all!(RC<SamplerTable>: Send, Sync);

impl<C: DescTable> DescRef for RC<C> {}

impl<C: DescTable> RC<C> {
	#[inline]
	pub fn new(slot: RCSlot<C::Slot, C::RCSlotsInterface>) -> Self {
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

impl<C: DescTable> Clone for RC<C> {
	fn clone(&self) -> Self {
		Self {
			slot: self.slot.clone(),
		}
	}
}

impl<C: DescTable> Hash for RC<C> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slot.hash(state)
	}
}

impl<C: DescTable> PartialEq<Self> for RC<C> {
	fn eq(&self, other: &Self) -> bool {
		self.slot == other.slot
	}
}

impl<C: DescTable> Eq for RC<C> {}
