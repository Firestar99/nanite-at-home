use crate::atomic_slots::RCSlot;
use crate::descriptor::descriptor_cpu_type::DescCpuType;
use crate::frame_in_flight::FrameInFlight;
use std::ops::Deref;
use vulkano_bindless_shaders::descriptor::{TransientDesc, WeakDesc};

#[derive(Clone)]
pub struct RCDesc<T: DescCpuType> {
	inner: RCSlot<T::TableType>,
}

impl<T: DescCpuType> RCDesc<T> {
	pub fn new(inner: RCSlot<T::TableType>) -> Self {
		Self { inner }
	}

	pub fn to_weak(&self) -> WeakDesc<T> {
		WeakDesc::new(self.inner.id(), self.inner.id())
	}

	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, T> {
		TransientDesc::new(self.inner.id(), frame)
	}
}

impl<T: DescCpuType> Deref for RCDesc<T> {
	type Target = T::CpuType;

	fn deref(&self) -> &Self::Target {
		T::deref_table(&self.inner)
	}
}
