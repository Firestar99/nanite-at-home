use crate::descriptor::Bindless;
use crate::platform::ash::Ash;
use crate::platform::ExecutingContext;
use ash::vk::{
	CommandPoolCreateFlags, CommandPoolCreateInfo, CommandPoolResetFlags, SemaphoreCreateInfo, SemaphoreType,
	SemaphoreTypeCreateInfo, SemaphoreWaitInfo,
};
use ash::Device;
use crossbeam_queue::SegQueue;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

#[derive(Debug, Copy, Clone)]
pub struct AshExecutionResource {
	pub command_pool: ash::vk::CommandPool,
	pub semaphore: ash::vk::Semaphore,
	pub timeline_value: u64,
}

impl AshExecutionResource {
	pub fn new(device: &Device) -> Self {
		unsafe {
			let timeline_value = 0;
			Self {
				command_pool: device
					.create_command_pool(
						&CommandPoolCreateInfo::default().flags(CommandPoolCreateFlags::TRANSIENT),
						None,
					)
					.unwrap(),
				semaphore: device
					.create_semaphore(
						&SemaphoreCreateInfo::default().push_next(
							&mut SemaphoreTypeCreateInfo::default()
								.semaphore_type(SemaphoreType::TIMELINE)
								.initial_value(timeline_value),
						),
						None,
					)
					.unwrap(),
				timeline_value,
			}
		}
	}

	pub fn increment_timeline_value(&mut self) -> u64 {
		self.timeline_value += 1;
		self.timeline_value
	}

	pub fn reset(&self, device: &Device) {
		unsafe {
			device
				.reset_command_pool(self.command_pool, CommandPoolResetFlags::RELEASE_RESOURCES)
				.unwrap();
		}
	}

	pub unsafe fn destroy(&self, device: &Device) {
		unsafe {
			device.destroy_command_pool(self.command_pool, None);
			device.destroy_semaphore(self.semaphore, None);
		}
	}
}

pub struct AshPooledExecutionResource {
	pub bindless: Arc<Bindless<Ash>>,
	pub inner: AshExecutionResource,
}

impl Deref for AshPooledExecutionResource {
	type Target = AshExecutionResource;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for AshPooledExecutionResource {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl Drop for AshPooledExecutionResource {
	fn drop(&mut self) {
		self.bindless.execution_manager.push(&self.bindless, self.inner)
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
			inner: reuse.unwrap_or_else(|| AshExecutionResource::new(&bindless.device)),
			bindless,
		}
	}

	fn push(&self, bindless: &Arc<Bindless<Ash>>, resource: AshExecutionResource) {
		resource.reset(&bindless.device);
		self.free_pool.push(resource);
	}

	pub fn destroy(&mut self, device: &Device) {
		unsafe {
			if let Some(resource) = self.free_pool.pop() {
				resource.destroy(device)
			}
		}
	}
}

pub struct AshExecutingContext<R> {
	resource: AshPooledExecutionResource,
	r: R,
}

impl<R> AshExecutingContext<R> {
	pub unsafe fn new(resource: AshPooledExecutionResource, r: R) -> Self {
		Self { resource, r }
	}
}

impl<R> Deref for AshExecutingContext<R> {
	type Target = AshPooledExecutionResource;
	fn deref(&self) -> &Self::Target {
		&self.resource
	}
}

unsafe impl<R: Send + Sync> ExecutingContext<Ash, R> for AshExecutingContext<R> {
	fn block_on(self) -> R {
		unsafe {
			let device = &self.resource.bindless.device;
			device
				.wait_semaphores(
					&SemaphoreWaitInfo::default()
						.semaphores(&[self.semaphore])
						.values(&[self.timeline_value]),
					!0,
				)
				.unwrap();
			self.r
		}
	}
}
