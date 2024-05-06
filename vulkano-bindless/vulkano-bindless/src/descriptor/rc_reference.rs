use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slots::RCSlot;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::{TransientDesc, WeakDesc};

pub struct RCDesc<D: DescTypeCpu + ?Sized> {
	inner: RCSlot<<D::ResourceTableCpu as ResourceTableCpu>::SlotType>,
}

impl<D: DescTypeCpu + ?Sized> RCDesc<D> {
	pub fn new(inner: RCSlot<<D::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> Self {
		Self { inner }
	}

	#[inline]
	pub fn id(&self) -> u32 {
		// we guarantee 32 bits is enough when constructing the resource tables
		*self.inner.id() as u32
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.inner.version()
	}

	pub fn to_weak(&self) -> WeakDesc<D> {
		WeakDesc::new(self.id(), self.version())
	}

	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, D> {
		let _ = frame;
		TransientDesc::new(self.id())
	}
}

impl<T: DescTypeCpu + ?Sized> Deref for RCDesc<T> {
	type Target = T::CpuType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.inner)
	}
}

impl<D: DescTypeCpu + ?Sized> Clone for RCDesc<D> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

impl<D: DescTypeCpu + ?Sized> Hash for RCDesc<D> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.inner.hash(state)
	}
}

impl<D: DescTypeCpu + ?Sized> PartialEq<Self> for RCDesc<D> {
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<D: DescTypeCpu + ?Sized> Eq for RCDesc<D> {}
