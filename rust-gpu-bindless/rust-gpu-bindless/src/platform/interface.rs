use crate::descriptor::{BufferSlot, DescriptorCounts, ImageSlot};

/// public interface for a Graphics API. Feel free to use as a base template for other traits.
pub unsafe trait Platform: 'static {
	type Entry: 'static;
	type Instance: 'static;
	type PhysicalDevice: 'static;
	type Device: Clone + 'static;
	type MemoryAllocator: 'static;
	type MemoryAllocation: 'static;
	type Buffer: 'static;
	type TypedBuffer<T: Send + Sync + ?Sized + 'static>: 'static;
	type Image: 'static;
	type ImageView: 'static;
	type Sampler: 'static;
	type DescriptorSet: Clone + 'static;
}

/// Internal interface for bindless API calls, may change at any time!
pub unsafe trait BindlessPlatform: Platform {
	unsafe fn update_after_bind_descriptor_limits(
		instance: &Self::Instance,
		phy: &Self::PhysicalDevice,
	) -> DescriptorCounts;

	unsafe fn destroy_buffers<'a>(
		device: &Self::Device,
		global_descriptor_set: &Self::DescriptorSet,
		buffers: impl Iterator<Item = &'a BufferSlot<Self>>,
	);

	unsafe fn destroy_images<'a>(
		device: &Self::Device,
		global_descriptor_set: &Self::DescriptorSet,
		images: impl Iterator<Item = ImageSlot<Self>>,
	);

	unsafe fn destroy_samplers<'a>(
		device: &Self::Device,
		global_descriptor_set: &Self::DescriptorSet,
		samplers: impl Iterator<Item = &'a Self::Sampler>,
	);
}
