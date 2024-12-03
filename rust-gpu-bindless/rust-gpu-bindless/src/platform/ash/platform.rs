use crate::descriptor::Bindless;
use crate::platform::ash::{AshAllocationError, AshExecutionManager};
use crate::platform::Platform;
use ash::vk::{PhysicalDeviceVulkan12Features, PipelineCache, ShaderStageFlags};
use gpu_allocator::vulkan::{Allocation, Allocator};
use parking_lot::lock_api::MutexGuard;
use parking_lot::{Mutex, RawMutex};
use static_assertions::assert_impl_all;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::{Arc, Weak};

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

pub struct Ash {
	pub create_info: Arc<AshCreateInfo>,
	pub execution_manager: AshExecutionManager,
}
assert_impl_all!(Bindless<Ash>: Send, Sync);

impl Ash {
	pub fn new(create_info: Arc<AshCreateInfo>, bindless: &Weak<Bindless<Self>>) -> Self {
		Ash {
			execution_manager: AshExecutionManager::new(bindless),
			create_info,
		}
	}
}

impl Deref for Ash {
	type Target = AshCreateInfo;

	fn deref(&self) -> &Self::Target {
		&self.create_info
	}
}

unsafe impl Platform for Ash {
	type PlatformCreateInfo = AshCreateInfo;
	/// # Safety
	/// UnsafeCell: Required to gain mutable access where it is safe to do so, see safety of interface methods.
	/// MaybeUninit: The Allocation is effectively always initialized, it only becomes uninit after running destroy.
	type MemoryAllocation = UnsafeCell<MaybeUninit<Allocation>>;
	type Buffer = ash::vk::Buffer;
	type TypedBuffer<T: Send + Sync + ?Sized> = Self::Buffer;
	type Image = ash::vk::Image;
	type ImageView = ash::vk::ImageView;
	type Sampler = ash::vk::Sampler;
	type AllocationError = AshAllocationError;
}

pub struct AshCreateInfo {
	pub entry: ash::Entry,
	pub instance: ash::Instance,
	pub physical_device: ash::vk::PhysicalDevice,
	pub device: ash::Device,
	pub memory_allocator: Option<Mutex<Allocator>>,
	pub shader_stages: ShaderStageFlags,
	pub queue_family_index: u32,
	pub queue: ash::vk::Queue,
	pub cache: Option<PipelineCache>,
	pub destroy: Option<Box<dyn FnOnce(&mut AshCreateInfo) + Send + Sync>>,
}

impl AshCreateInfo {
	pub fn memory_allocator(&self) -> MutexGuard<'_, RawMutex, Allocator> {
		self.memory_allocator.as_ref().unwrap().lock()
	}
}

impl Drop for AshCreateInfo {
	fn drop(&mut self) {
		if let Some(destroy) = self.destroy.take() {
			destroy(self);
		}
	}
}

pub struct RunOnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> RunOnDrop<F> {
	pub fn new(f: F) -> Self {
		Self(Some(f))
	}

	pub fn take(mut self) -> F {
		self.0.take().unwrap()
	}
}

impl<F: FnOnce()> Drop for RunOnDrop<F> {
	fn drop(&mut self) {
		if let Some(f) = self.0.take() {
			f()
		}
	}
}
