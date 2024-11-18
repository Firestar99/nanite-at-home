use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::DrainFlushQueue;
use crate::descriptor::{
	BindlessBufferCreateInfo, BindlessCreateInfo, BufferInterface, DescriptorCounts, ImageInterface, SamplerInterface,
};
use crate::platform::Platform;
use std::sync::Arc;

/// Internal interface for bindless API calls, may change at any time!
pub unsafe trait BindlessPlatform: Platform {
	unsafe fn update_after_bind_descriptor_limits(ci: &Arc<BindlessCreateInfo<Self>>) -> DescriptorCounts;

	unsafe fn create_descriptor_set(ci: &Arc<BindlessCreateInfo<Self>>) -> Self::BindlessDescriptorSet;

	/// Update the [`BindlessDescriptorSet`] with these changed buffers, images and samplers.
	///
	/// # Safety
	/// Must be called while holding the associated [`TableSync`]'s [`FlushGuard`].
	///
	/// [`TableSync`]: crate::backend::table::TableSync
	/// [`FlushGuard`]: crate::backend::table::FlushGuard
	unsafe fn update_descriptor_set(
		ci: &Arc<BindlessCreateInfo<Self>>,
		set: &Self::BindlessDescriptorSet,
		buffers: DrainFlushQueue<BufferInterface<Self>>,
		images: DrainFlushQueue<ImageInterface<Self>>,
		samplers: DrainFlushQueue<SamplerInterface<Self>>,
	);

	unsafe fn destroy_descriptor_set(ci: &Arc<BindlessCreateInfo<Self>>, set: Self::BindlessDescriptorSet);

	unsafe fn alloc_buffer(
		ci: &Arc<BindlessCreateInfo<Self>>,
		create_info: &BindlessBufferCreateInfo,
		size: u64,
	) -> Result<(Self::Buffer, Self::MemoryAllocation), Self::AllocationError>;

	/// Turn the [`MemoryAllocation`] into a Slab. You may assume that the [`MemoryAllocation`] is mappable and has
	/// either [`BindlessBufferUsage::MAP_WRITE`] or [`BindlessBufferUsage::MAP_READ`]. You also have exclusive access
	/// to the [`MemoryAllocation`].
	unsafe fn memory_allocation_to_slab<'a>(
		memory_allocation: &'a Self::MemoryAllocation,
	) -> &'a mut (impl presser::Slab + 'a);

	unsafe fn reinterpet_ref_buffer<T: Send + Sync + ?Sized + 'static>(buffer: &Self::Buffer) -> &Self::TypedBuffer<T>;

	/// Destroy specified buffers. You have exclusive access to the associated [`BufferSlot`]s, even if they are just
	/// passed by standard reference. After this method call returns, the [`BufferSlot`] will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_buffers<'a>(
		ci: &Arc<BindlessCreateInfo<Self>>,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	);

	/// Destroy specified images. You have exclusive access to the associated [`ImageSlot`]s, even if they are just
	/// passed by standard reference. After this method call returns, the [`ImageSlot`] will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_images<'a>(
		ci: &Arc<BindlessCreateInfo<Self>>,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		images: impl DescriptorIndexIterator<'a, ImageInterface<Self>>,
	);

	/// Destroy specified Samplers. You have exclusive access to the associated Samplers, even if they are just
	/// passed by standard reference. After this method call returns, the Samplers will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_samplers<'a>(
		ci: &Arc<BindlessCreateInfo<Self>>,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	);
}
