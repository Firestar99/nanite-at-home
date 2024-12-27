use crate::descriptor::{Bindless, BindlessFrame};
use crate::platform::ash::Ash;
use crate::platform::PendingExecution;
use ash::vk::{
	CommandPoolCreateFlags, CommandPoolCreateInfo, CommandPoolResetFlags, SemaphoreCreateInfo, SemaphoreType,
	SemaphoreTypeCreateInfo,
};
use ash::Device;
use crossbeam_queue::SegQueue;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};

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
	/// To ensure no racing may happen, `wakers` must be held while this is checked for consistent results.
	completed: AtomicBool,
	wakers: Mutex<SmallVec<[Waker; 1]>>,
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

	pub fn new(frame: Arc<BindlessFrame<Ash>>, resource: AshExecutionResource) -> Self {
		Self {
			frame,
			resource,
			completed: AtomicBool::new(false),
			wakers: Mutex::new(SmallVec::new()),
		}
	}

	pub fn completed(&self) -> bool {
		self.completed.load(Relaxed)
	}

	fn set_completed(&self) {
		let wakers = {
			let mut guard = self.wakers.lock();
			// must be set while holding `wakers` to prevent races
			self.completed.store(true, Relaxed);
			mem::replace(&mut *guard, SmallVec::new())
		};
		for x in wakers {
			x.wake();
		}
	}
}

impl Debug for AshExecution {
	fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
		f.debug_tuple("AshExecution")
			.field(if self.completed() { &"completed" } else { &"pending" })
			.finish()
	}
}

impl Future for AshExecution {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		// fast fail
		if self.completed.load(Relaxed) {
			Poll::Ready(())
		} else {
			let mut guard = self.wakers.lock();
			// consistent check
			if self.completed.load(Relaxed) {
				Poll::Ready(())
			} else {
				guard.push(cx.waker().clone());
				Poll::Pending
			}
		}
	}
}

impl Drop for AshExecution {
	fn drop(&mut self) {
		self.frame
			.bindless
			.execution_manager
			.push_to_free_pool(&self.frame.bindless, self.resource.clone())
	}
}

pub struct AshExecutionManager {
	bindless: Weak<Bindless<Ash>>,
	free_pool: SegQueue<AshExecutionResource>,
	submit_for_waiting: SegQueue<Arc<AshExecution>>,
}

impl AshExecutionManager {
	pub fn new(bindless: &Weak<Bindless<Ash>>) -> Self {
		Self {
			bindless: bindless.clone(),
			free_pool: SegQueue::new(),
			submit_for_waiting: SegQueue::new(),
		}
	}

	pub fn bindless(&self) -> Arc<Bindless<Ash>> {
		self.bindless.upgrade().expect("bindless was freed")
	}

	pub fn new_execution(&self) -> Arc<AshExecution> {
		let bindless = self.bindless();
		Arc::new(AshExecution::new(
			bindless.frame(),
			self.free_pool
				.pop()
				.unwrap_or_else(|| AshExecutionResource::new(&bindless.device)),
		))
	}

	fn push_to_free_pool(&self, bindless: &Arc<Bindless<Ash>>, mut resource: AshExecutionResource) {
		resource.reset(&bindless.device);
		self.free_pool.push(resource);
	}

	/// # Safety
	/// must only submit an execution exactly once
	pub(super) unsafe fn submit_for_waiting(&self, execution: Arc<AshExecution>) {
		self.submit_for_waiting.push(execution);
		todo!("notify wait thread")
	}

	pub fn destroy(&mut self, device: &Device) {
		unsafe {
			if let Some(resource) = self.free_pool.pop() {
				resource.destroy(device)
			}
		}
	}
}

#[derive(Clone)]
pub struct AshPendingExecution {
	execution: Option<Weak<AshExecution>>,
}

impl AshPendingExecution {
	pub fn new(execution: &Arc<AshExecution>) -> Self {
		Self {
			execution: Some(Arc::downgrade(execution)),
		}
	}

	pub fn upgrade_ash_resource(&self) -> Option<Arc<AshExecution>> {
		self.execution.as_ref().and_then(|weak| weak.upgrade())
	}
}

unsafe impl PendingExecution<Ash> for AshPendingExecution {
	#[inline]
	fn completed() -> Self {
		Self { execution: None }
	}
}

impl Future for AshPendingExecution {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		if let Some(execution) = self.upgrade_ash_resource() {
			// fast fail
			if execution.completed.load(Relaxed) {
				Poll::Ready(())
			} else {
				let mut guard = execution.wakers.lock();
				// consistent check
				if execution.completed.load(Relaxed) {
					Poll::Ready(())
				} else {
					guard.push(cx.waker().clone());
					Poll::Pending
				}
			}
		} else {
			Poll::Ready(())
		}
	}
}
