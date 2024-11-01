use crate::backend::range_set::DescriptorIndexIterator;
use crate::descriptor::{BindlessCreateInfo, BufferInterface, DescriptorCounts, ImageInterface, SamplerInterface};

/// public interface for a Graphics API. Feel free to use as a base template for other traits.
pub unsafe trait Platform: Sized + Send + Sync + 'static {
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
	type AllocationError: 'static;
	type DescriptorSet: Clone + 'static;
}

/// Internal interface for bindless API calls, may change at any time!
pub unsafe trait BindlessPlatform: Platform {
	unsafe fn update_after_bind_descriptor_limits(ci: &BindlessCreateInfo<Self>) -> DescriptorCounts;

	unsafe fn reinterpet_ref_buffer<T: Send + Sync + ?Sized + 'static>(buffer: &Self::Buffer) -> &Self::TypedBuffer<T>;

	unsafe fn destroy_buffers<'a>(
		ci: &BindlessCreateInfo<Self>,
		global_descriptor_set: &Self::DescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	);

	unsafe fn destroy_images<'a>(
		ci: &BindlessCreateInfo<Self>,
		global_descriptor_set: &Self::DescriptorSet,
		images: impl DescriptorIndexIterator<'a, ImageInterface<Self>>,
	);

	unsafe fn destroy_samplers<'a>(
		ci: &BindlessCreateInfo<Self>,
		global_descriptor_set: &Self::DescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	);
}
