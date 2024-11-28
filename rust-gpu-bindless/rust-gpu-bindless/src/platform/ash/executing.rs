use crate::descriptor::Bindless;
use crate::platform::ash::Ash;
use crate::platform::ExecutingCommandBuffer;
use ash::vk::{
	CommandPoolCreateFlags, CommandPoolCreateInfo, CommandPoolResetFlags, FenceCreateInfo, SemaphoreCreateInfo,
};
use ash::Device;
use crossbeam_queue::SegQueue;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

#[derive(Debug, Copy, Clone)]
pub struct AshExecutionResource {
	pub command_pool: ash::vk::CommandPool,
	pub fence: ash::vk::Fence,
	pub semaphore: ash::vk::Semaphore,
}

impl AshExecutionResource {
	pub fn new(device: &Device) -> Self {
		unsafe {
			Self {
				command_pool: device
					.create_command_pool(
						&CommandPoolCreateInfo::default().flags(CommandPoolCreateFlags::TRANSIENT),
						None,
					)
					.unwrap(),
				fence: device.create_fence(&FenceCreateInfo::default(), None).unwrap(),
				semaphore: device.create_semaphore(&SemaphoreCreateInfo::default(), None).unwrap(),
			}
		}
	}

	pub fn reset(&self, device: &Device) {
		unsafe {
			device
				.reset_command_pool(self.command_pool, CommandPoolResetFlags::RELEASE_RESOURCES)
				.unwrap();
			device.reset_fences(&[self.fence]).unwrap();
		}
	}

	pub unsafe fn destroy(&self, device: &Device) {
		unsafe {
			device.destroy_command_pool(self.command_pool, None);
			device.destroy_fence(self.fence, None);
			device.destroy_semaphore(self.semaphore, None);
		}
	}
}

pub struct AshPooledExecutionResource {
	pub bindless: Arc<Bindless<Ash>>,
	pub resource: AshExecutionResource,
}

impl Deref for AshPooledExecutionResource {
	type Target = AshExecutionResource;

	fn deref(&self) -> &Self::Target {
		&self.resource
	}
}

impl DerefMut for AshPooledExecutionResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.resource
	}
}

impl Drop for AshPooledExecutionResource {
	fn drop(&mut self) {
		self.bindless.execution_manager.push(&self.bindless, self.resource)
	}
}

pub struct AshExecutionManager {
	pub bindless: Weak<Bindless<Ash>>,
	free_pool: SegQueue<AshExecutionResource>,
}

impl AshExecutionManager {
	pub fn new(bindless: &Weak<Bindless<Ash>>) -> Self {
		Self {
			bindless: bindless.clone(),
			free_pool: SegQueue::new(),
		}
	}

	pub fn pop(&self) -> AshPooledExecutionResource {
		let bindless = self.bindless.upgrade().expect("bindless was freed");
		let reuse = self.free_pool.pop();
		AshPooledExecutionResource {
			resource: reuse.unwrap_or_else(|| AshExecutionResource::new(&bindless.device)),
			bindless,
		}
	}

	fn push(&self, bindless: &Arc<Bindless<Ash>>, resource: AshExecutionResource) {
		resource.reset(&bindless.device);
		self.free_pool.push(resource);
	}
}

pub struct AshExecutingCommandBuffer {
	resource: AshPooledExecutionResource,
}

impl AshExecutingCommandBuffer {
	pub unsafe fn new(resource: AshPooledExecutionResource) -> Self {
		Self { resource }
	}
}

impl Deref for AshExecutingCommandBuffer {
	type Target = AshPooledExecutionResource;
	fn deref(&self) -> &Self::Target {
		&self.resource
	}
}

unsafe impl ExecutingCommandBuffer<Ash> for AshExecutingCommandBuffer {}
