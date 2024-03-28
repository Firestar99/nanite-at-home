use crate::atomic_slots::RCSlot;
use crate::descriptor::table_type::DescTableType;
use crate::frame_in_flight::FrameInFlight;
use vulkano_bindless_shaders::descriptor::{TransientDesc, WeakDesc};

#[derive(Clone)]
pub struct RCDesc<T: DescTableType> {
	inner: RCSlot<T::CpuType>,
}

impl<T: DescTableType> RCDesc<T> {
	pub fn new(inner: RCSlot<T::CpuType>) -> Self {
		Self { inner }
	}

	pub fn to_weak(&self) -> WeakDesc<T> {
		WeakDesc::new(self.inner.id(), self.inner.id())
	}

	pub fn to_transient<'a>(&self, frame: FrameInFlight<'a>) -> TransientDesc<'a, T> {
		TransientDesc::new(self.inner.id(), frame)
	}
}
