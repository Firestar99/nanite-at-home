use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::DeviceSize;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter};

use crate::space::Init;
use crate::space::renderer::frame_in_flight::{FrameInFlight, SeedInFlight};
use crate::space::renderer::frame_in_flight::resource::ResourceInFlight;
use crate::vulkan::concurrent_sharing;

pub struct UniformInFlight<T: BufferContents> {
	sub: ResourceInFlight<Subbuffer<T>>,
}

impl<T: BufferContents> UniformInFlight<T> {
	pub fn new(init: &Arc<Init>, seed: impl Into<SeedInFlight>, dedicated_alloc: bool) -> Self {
		fn inner<T: BufferContents>(init: &Arc<Init>, seed: SeedInFlight, dedicated_alloc: bool) -> UniformInFlight<T> {
			let buffer = Buffer::new_slice::<T>(
				&init.memory_allocator,
				BufferCreateInfo {
					usage: BufferUsage::UNIFORM_BUFFER,
					sharing: concurrent_sharing(&[
						&init.queues.client.graphics_main,
						&init.queues.client.async_compute,
					]),
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					allocate_preference: if dedicated_alloc {
						MemoryAllocatePreference::AlwaysAllocate
					} else {
						MemoryAllocatePreference::Unknown
					},
					..AllocationCreateInfo::default()
				},
				seed.frames_in_flight() as DeviceSize,
			).unwrap();

			// this will clone the buffer once too many times, and drop it afterwards
			UniformInFlight {
				sub: ResourceInFlight::new(seed, |i| buffer.clone().index(i.index() as DeviceSize)),
			}
		}
		inner(init, seed.into(), dedicated_alloc)
	}

	pub fn upload(&self, frame: FrameInFlight, data: T) {
		let sub = &self.sub.index(frame);
		*sub.write().unwrap() = data;
	}
}

impl<T: BufferContents> Deref for UniformInFlight<T> {
	type Target = ResourceInFlight<Subbuffer<T>>;

	fn deref(&self) -> &Self::Target {
		&self.sub
	}
}

impl<T: BufferContents> DerefMut for UniformInFlight<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.sub
	}
}

impl<T: BufferContents> From<&UniformInFlight<T>> for SeedInFlight {
	fn from(value: &UniformInFlight<T>) -> Self {
		value.seed()
	}
}
