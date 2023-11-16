use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use smallvec::SmallVec;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryAllocator, MemoryTypeFilter};
use vulkano::sync::Sharing;
use vulkano::DeviceSize;

use crate::space::renderer::frame_in_flight::resource::ResourceInFlight;
use crate::space::renderer::frame_in_flight::{FrameInFlight, SeedInFlight};

pub struct UniformInFlight<T: BufferContents> {
	sub: ResourceInFlight<Subbuffer<T>>,
}

impl<T: BufferContents> UniformInFlight<T> {
	pub fn new(
		allocator: Arc<dyn MemoryAllocator>,
		sharing: Sharing<SmallVec<[u32; 4]>>,
		seed: impl Into<SeedInFlight>,
		dedicated_alloc: bool,
	) -> Self {
		fn inner<T: BufferContents>(
			allocator: Arc<dyn MemoryAllocator>,
			sharing: Sharing<SmallVec<[u32; 4]>>,
			seed: SeedInFlight,
			dedicated_alloc: bool,
		) -> UniformInFlight<T> {
			let buffer = Buffer::new_slice::<T>(
				allocator,
				BufferCreateInfo {
					usage: BufferUsage::UNIFORM_BUFFER,
					sharing,
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
			)
			.unwrap();

			// this will clone the buffer once too many times, and drop it afterwards
			UniformInFlight {
				sub: ResourceInFlight::new(seed, |i| buffer.clone().index(i.index() as DeviceSize)),
			}
		}
		inner(allocator, sharing, seed.into(), dedicated_alloc)
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
