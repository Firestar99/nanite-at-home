use crate::descriptor::Bindless;
use crate::platform::Platform;
use ash::vk::PhysicalDeviceVulkan12Features;
use gpu_allocator::vulkan::{Allocation, Allocator};
use parking_lot::Mutex;
use static_assertions::assert_impl_all;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;

mod bindless;

pub use bindless::*;

pub fn required_features_vk12() -> PhysicalDeviceVulkan12Features<'static> {
	PhysicalDeviceVulkan12Features::default()
		.vulkan_memory_model(true)
		.runtime_descriptor_array(true)
		.descriptor_binding_update_unused_while_pending(true)
		.descriptor_binding_partially_bound(true)
		.descriptor_binding_storage_buffer_update_after_bind(true)
		.descriptor_binding_sampled_image_update_after_bind(true)
		.descriptor_binding_storage_image_update_after_bind(true)
		.descriptor_binding_uniform_buffer_update_after_bind(true)
}

pub struct Ash;
assert_impl_all!(Bindless<Ash>: Send, Sync);

unsafe impl Platform for Ash {
	type Entry = ash::Entry;
	type Instance = ash::Instance;
	type PhysicalDevice = ash::vk::PhysicalDevice;
	type Device = ash::Device;
	type MemoryAllocator = Mutex<Allocator>;
	/// # Safety
	/// UnsafeCell: Required to gain mutable access where it is safe to do so, see safety of interface methods.
	/// MaybeUninit: The Allocation is effectively always initialized, it only becomes uninit after running destroy.
	type MemoryAllocation = UnsafeCell<MaybeUninit<Allocation>>;
	type Buffer = ash::vk::Buffer;
	type TypedBuffer<T: Send + Sync + ?Sized + 'static> = Self::Buffer;
	type Image = ash::vk::Image;
	type ImageView = ash::vk::ImageView;
	type Sampler = ash::vk::Sampler;
	type AllocationError = AshAllocationError;
}
