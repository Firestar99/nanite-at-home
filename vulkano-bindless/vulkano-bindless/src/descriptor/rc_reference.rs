use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::frame_in_flight::FrameInFlight;
use crate::rc_slots::RCSlot;
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::{TransientDesc, WeakDesc};

#[derive(Clone)]
pub struct RCDesc<D: DescTypeCpu> {
	inner: RCSlot<<D::ResourceTableCpu as ResourceTableCpu>::SlotType>,
}

impl<D: DescTypeCpu> RCDesc<D> {
	pub fn new(inner: RCSlot<<D::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> Self {
		Self { inner }
	}

	#[inline]
	pub fn id(&self) -> u32 {
		// we guarantee 32 bits is enough when constructing the resource tables
		self.inner.id() as u32
	}

	#[inline]
	pub fn version(&self) -> u32 {
		self.inner.version()
	}

	pub fn to_weak(&self) -> WeakDesc<D> {
		WeakDesc::new(self.id(), self.version())
	}

	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, D> {
		TransientDesc::new(self.id(), frame)
	}
}

impl<T: DescTypeCpu> Deref for RCDesc<T> {
	type Target = T::CpuType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.inner)
	}
}
