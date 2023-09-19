use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::DeviceSize;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter};
use vulkano::sync::Sharing;

use crate::space::Init;
use crate::space::renderer::frame_in_flight::{FrameInFlight, SeedInFlight};
use crate::space::renderer::frame_in_flight::resource::ResourceInFlight;
use crate::vulkan::unique_queue_families;

struct UniformDataInFlight<T: BufferContents> {
	sub: ResourceInFlight<Subbuffer<T>>,
}

impl<T: BufferContents> UniformDataInFlight<T> {
	fn new(init: &Arc<Init>, seed: SeedInFlight) -> Self {
		let buffer = Buffer::new_slice::<T>(
			&init.memory_allocator,
			BufferCreateInfo {
				usage: BufferUsage::UNIFORM_BUFFER,
				sharing: Sharing::Concurrent(unique_queue_families(&[
					&init.queues.client.graphics_main,
					&init.queues.client.async_compute,
				])),
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				// it's small but read so many times, let's keep it dedicated
				allocate_preference: MemoryAllocatePreference::AlwaysAllocate,
				..AllocationCreateInfo::default()
			},
			seed.frames_in_flight() as DeviceSize,
		).unwrap();

		// this will clone the buffer once too many times, and drop it afterwards
		Self {
			sub: ResourceInFlight::new(seed, |i| buffer.clone().index(i.index() as DeviceSize)),
		}
	}

	fn upload(&self, frame: FrameInFlight, data: T) {
		let sub = &self.sub.index(frame);
		*sub.write().unwrap() = data;
	}
}

impl<T: BufferContents> Deref for UniformDataInFlight<T> {
	type Target = ResourceInFlight<Subbuffer<T>>;

	fn deref(&self) -> &Self::Target {
		&self.sub
	}
}

impl<T: BufferContents> DerefMut for UniformDataInFlight<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.sub
	}
}
