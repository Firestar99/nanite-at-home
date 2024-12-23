use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::DrainFlushQueue;
use crate::descriptor::{
	Bindless, BindlessBufferCreateInfo, BindlessImageCreateInfo, BufferInterface, DescriptorCounts, ImageInterface,
	SamplerInterface,
};
use crate::platform::Platform;
use rust_gpu_bindless_shaders::descriptor::ImageType;

/// Internal interface for bindless API calls, may change at any time!
pub unsafe trait BindlessPlatform: Platform {
	type BindlessDescriptorSet: 'static + Send + Sync;

	/// Create an [`Self::Platform`] from the supplied [`Self::PlatformCreateInfo`]. Typically, [`Self::PlatformCreateInfo`] wrap the
	/// implementation's instance, device and other objects required to be initialized by the end user.
	/// [`Self::Platform`] either is the same as or derefs to the original [`Self::PlatformCreateInfo`] and has some additional
	/// members that will be initialized in this function.
	unsafe fn create_platform(
		create_info: Self::PlatformCreateInfo,
		bindless_cyclic: &std::sync::Weak<Bindless<Self>>,
	) -> Self;

	unsafe fn update_after_bind_descriptor_limits(&self) -> DescriptorCounts;

	unsafe fn create_descriptor_set(&self, counts: DescriptorCounts) -> Self::BindlessDescriptorSet;

	/// Bindless has been fully initialized but not yet returned to the end user. Feel free to do any required
	/// modifications or buffer allocations here.
	unsafe fn bindless_initialized(&self, bindless: &mut Bindless<Self>);

	/// Update the [`BindlessDescriptorSet`] with these changed buffers, images and samplers.
	///
	/// # Safety
	/// Must be called while holding the associated [`TableSync`]'s [`FlushGuard`].
	///
	/// [`TableSync`]: crate::backing::table::TableSync
	/// [`FlushGuard`]: crate::backing::table::FlushGuard
	unsafe fn update_descriptor_set(
		&self,
		set: &Self::BindlessDescriptorSet,
		buffers: DrainFlushQueue<BufferInterface<Self>>,
		images: DrainFlushQueue<ImageInterface<Self>>,
		samplers: DrainFlushQueue<SamplerInterface<Self>>,
	);

	unsafe fn destroy_descriptor_set(&self, set: Self::BindlessDescriptorSet);

	unsafe fn alloc_buffer(
		&self,
		create_info: &BindlessBufferCreateInfo,
		size: u64,
	) -> Result<(Self::Buffer, Self::MemoryAllocation), Self::AllocationError>;

	unsafe fn alloc_image<T: ImageType>(
		&self,
		create_info: &BindlessImageCreateInfo<T>,
	) -> Result<(Self::Image, Self::ImageView, Self::MemoryAllocation), Self::AllocationError>;

	/// Turn the [`MemoryAllocation`] into a Slab. You may assume that the [`MemoryAllocation`] is mappable and has
	/// either [`BindlessBufferUsage::MAP_WRITE`] or [`BindlessBufferUsage::MAP_READ`]. You also have exclusive access
	/// to the [`MemoryAllocation`].
	unsafe fn memory_allocation_to_slab<'a>(
		memory_allocation: &'a Self::MemoryAllocation,
	) -> &'a mut (impl presser::Slab + 'a);

	/// Destroy specified buffers. You have exclusive access to the associated [`BufferSlot`]s, even if they are just
	/// passed by standard reference. After this method call returns, the [`BufferSlot`] will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_buffers<'a>(
		&self,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		buffers: impl DescriptorIndexIterator<'a, BufferInterface<Self>>,
	);

	/// Destroy specified images. You have exclusive access to the associated [`ImageSlot`]s, even if they are just
	/// passed by standard reference. After this method call returns, the [`ImageSlot`] will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_images<'a>(
		&self,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		images: impl DescriptorIndexIterator<'a, ImageInterface<Self>>,
	);

	/// Destroy specified Samplers. You have exclusive access to the associated Samplers, even if they are just
	/// passed by standard reference. After this method call returns, the Samplers will be dropped and otherwise
	/// not accessed anymore.
	unsafe fn destroy_samplers<'a>(
		&self,
		global_descriptor_set: &Self::BindlessDescriptorSet,
		samplers: impl DescriptorIndexIterator<'a, SamplerInterface<Self>>,
	);
}
