use crate::descriptor::Bindless;
use crate::platform::ash::{AshAllocationError, AshExecutionManager};
use crate::platform::Platform;
use ash::vk::{PipelineCache, ShaderStageFlags};
use gpu_allocator::vulkan::{Allocation, Allocator};
use parking_lot::lock_api::MutexGuard;
use parking_lot::{Mutex, RawMutex};
use static_assertions::assert_impl_all;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::Weak;

pub struct Ash {
	pub create_info: AshCreateInfo,
	pub execution_manager: AshExecutionManager,
}
assert_impl_all!(Bindless<Ash>: Send, Sync);

impl Ash {
	pub fn new(create_info: AshCreateInfo, bindless: &Weak<Bindless<Self>>) -> Self {
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

impl Drop for Ash {
	fn drop(&mut self) {
		self.execution_manager.destroy(&self.create_info.device);
	}
}

unsafe impl Platform for Ash {
	type PlatformCreateInfo = AshCreateInfo;
	/// # Safety
	/// UnsafeCell: Required to gain mutable access where it is safe to do so, see safety of interface methods.
	/// MaybeUninit: The Allocation is effectively always initialized, it only becomes uninit after running destroy.
	type MemoryAllocation = AshMemoryAllocation;
	type Buffer = ash::vk::Buffer;
	type Image = ash::vk::Image;
	type ImageView = Option<ash::vk::ImageView>;
	type Sampler = ash::vk::Sampler;
	type AllocationError = AshAllocationError;
}

/// Wraps gpu-allocator's MemoryAllocation to be able to [`Option::take`] it on drop, but saving the enum flag byte
/// with [`MaybeUninit`]
#[derive(Debug)]
pub struct AshMemoryAllocation(UnsafeCell<MaybeUninit<Allocation>>);

impl AshMemoryAllocation {
	/// Create a AshMemoryAllocation from a gpu-allocator Allocation
	///
	/// # Safety
	/// You must [`Self::take`] the Allocation and deallocate manually before dropping self
	pub unsafe fn new(allocation: Allocation) -> Self {
		Self(UnsafeCell::new(MaybeUninit::new(allocation)))
	}

	/// Get exclusive mutable access to the Allocation
	///
	/// # Safety
	/// You must ensure you have exclusive mutable access to the Allocation
	pub unsafe fn get_mut(&self) -> &mut Allocation {
		unsafe { (&mut *self.0.get()).assume_init_mut() }
	}

	/// Take the allocation
	///
	/// # Safety
	/// Once the allocation was taken, you must only drop self, any other action is unsafe
	pub unsafe fn take(&self) -> Allocation {
		unsafe { (&mut *self.0.get()).assume_init_read() }
	}
}

/// Safety: MemoryAllocation is safety Send and Sync, will only uninit on drop
unsafe impl Send for AshMemoryAllocation {}
unsafe impl Sync for AshMemoryAllocation {}

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
