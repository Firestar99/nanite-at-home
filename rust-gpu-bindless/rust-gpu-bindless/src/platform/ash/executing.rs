use crate::descriptor::{Bindless, BindlessFrame};
use crate::platform::ash::Ash;
use crate::platform::ExecutingContext;
use ash::vk::{
	CommandPoolCreateFlags, CommandPoolCreateInfo, CommandPoolResetFlags, SemaphoreCreateInfo, SemaphoreType,
	SemaphoreTypeCreateInfo, SemaphoreWaitInfo,
};
use ash::Device;
use crossbeam_queue::SegQueue;
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
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
				timeline_value: timeline_value + 1,
			}
		}
	}

	pub fn reset(&mut self, device: &Device) {
		unsafe {
			device
				.reset_command_pool(self.command_pool, CommandPoolResetFlags::RELEASE_RESOURCES)
				.unwrap();
			self.timeline_value += 1;
		}
	}

	pub unsafe fn destroy(&self, device: &Device) {
		unsafe {
			device.destroy_command_pool(self.command_pool, None);
			device.destroy_semaphore(self.semaphore, None);
		}
	}
}

pub struct AshExecution {
	frame: Arc<BindlessFrame<Ash>>,
	resource: AshExecutionResource,
}

impl AshExecution {
	#[inline]
	pub fn frame(&self) -> &Arc<BindlessFrame<Ash>> {
		&self.frame
	}

	#[inline]
	pub fn bindless(&self) -> &Arc<Bindless<Ash>> {
		&self.frame.bindless
	}

	#[inline]
	pub fn resource(&self) -> &AshExecutionResource {
		&self.resource
	}
}

impl Drop for AshExecution {
	fn drop(&mut self) {
		self.frame
			.bindless
			.execution_manager
			.push(&self.frame.bindless, self.resource.clone())
	}
}

pub struct AshExecutionManager {
	bindless: Weak<Bindless<Ash>>,
	free_pool: SegQueue<AshExecutionResource>,
}

impl AshExecutionManager {
	pub fn new(bindless: &Weak<Bindless<Ash>>) -> Self {
		Self {
			bindless: bindless.clone(),
			free_pool: SegQueue::new(),
		}
	}

	pub fn bindless(&self) -> Arc<Bindless<Ash>> {
		self.bindless.upgrade().expect("bindless was freed")
	}

	pub fn pop(&self) -> Arc<AshExecution> {
		let bindless = self.bindless();
		Arc::new(AshExecution {
			resource: self
				.free_pool
				.pop()
				.unwrap_or_else(|| AshExecutionResource::new(&bindless.device)),
			frame: bindless.frame(),
		})
	}

	fn push(&self, bindless: &Arc<Bindless<Ash>>, mut resource: AshExecutionResource) {
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
	execution: Arc<AshExecution>,
	r: R,
}

impl<R> AshExecutingContext<R> {
	pub unsafe fn new(execution: Arc<AshExecution>, r: R) -> Self {
		Self { execution, r }
	}

	pub unsafe fn ash_resource(&self) -> &Arc<AshExecution> {
		&self.execution
	}
}

unsafe impl<R: Send + Sync> ExecutingContext<Ash, R> for AshExecutingContext<R> {
	fn block_on(self) -> R {
		unsafe {
			let device = &self.execution.frame.bindless.device;
			device
				.wait_semaphores(
					&SemaphoreWaitInfo::default()
						.semaphores(&[self.execution.resource.semaphore])
						.values(&[self.execution.resource.timeline_value]),
					!0,
				)
				.unwrap();
			self.r
		}
	}
}
