use crate::descriptor::{Bindless, BindlessFrame, WeakBindless};
use crate::platform::PendingExecution;
use crate::platform::ash::{Ash, AshCreateInfo, DeviceExt};
use ash::Device;
use ash::prelude::VkResult;
use ash::vk::{
	CommandBufferAllocateInfo, CommandBufferLevel, CommandPoolCreateFlags, CommandPoolCreateInfo,
	CommandPoolResetFlags, SemaphoreCreateInfo, SemaphoreSignalInfo, SemaphoreType, SemaphoreTypeCreateInfo,
	SemaphoreWaitFlags, SemaphoreWaitInfo,
};
use crossbeam_queue::SegQueue;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};
use std::thread;

pub fn create_timeline_semaphore(device: &Device, timeline_value: u64) -> VkResult<ash::vk::Semaphore> {
	unsafe {
		device.create_semaphore(
			&SemaphoreCreateInfo::default().push_next(
				&mut SemaphoreTypeCreateInfo::default()
					.semaphore_type(SemaphoreType::TIMELINE)
					.initial_value(timeline_value),
			),
			None,
		)
	}
}

#[derive(Debug, Clone)]
pub struct AshExecutionResource {
	pub command_pool: ash::vk::CommandPool,
	pub command_buffer: ash::vk::CommandBuffer,
	pub semaphore: ash::vk::Semaphore,
	pub timeline_value: u64,
}

impl AshExecutionResource {
	pub fn new(device: &Device) -> VkResult<Self> {
		unsafe {
			let timeline_value = 0;
			let command_pool = device.create_command_pool(
				&CommandPoolCreateInfo::default().flags(CommandPoolCreateFlags::TRANSIENT),
				None,
			)?;
			let command_buffer = device.allocate_command_buffer(
				&CommandBufferAllocateInfo::default()
					.command_pool(command_pool)
					.level(CommandBufferLevel::PRIMARY)
					.command_buffer_count(1),
			)?;
			Ok(Self {
				command_pool,
				command_buffer,
				semaphore: create_timeline_semaphore(device, timeline_value)?,
				timeline_value: timeline_value + 1,
			})
		}
	}

	pub fn reset(&mut self, device: &Device) {
		unsafe {
			device
				.reset_command_pool(self.command_pool, CommandPoolResetFlags::empty())
				.unwrap();
			self.timeline_value += 1;
		}
	}

	pub unsafe fn destroy(&self, device: &Device) {
		unsafe {
			device.free_command_buffers(self.command_pool, &[self.command_buffer]);
			device.destroy_command_pool(self.command_pool, None);
			device.destroy_semaphore(self.semaphore, None);
		}
	}
}

pub struct AshExecution {
	bindless: Bindless<Ash>,
	resource: AshExecutionResource,
	/// To ensure no racing may happen, `wakers` must be held while this is checked for consistent results.
	completed: AtomicBool,
	mutex: Mutex<MutexedAshExecution>,
}

pub struct MutexedAshExecution {
	frame: Option<BindlessFrame<Ash>>,
	wakers: SmallVec<[Waker; 1]>,
}

impl AshExecution {
	#[inline]
	pub fn bindless(&self) -> &Bindless<Ash> {
		&self.bindless
	}

	#[inline]
	pub fn resource(&self) -> &AshExecutionResource {
		&self.resource
	}

	pub fn new(resource: AshExecutionResource, frame: BindlessFrame<Ash>) -> Self {
		Self {
			bindless: frame.bindless.clone(),
			resource,
			completed: AtomicBool::new(false),
			mutex: Mutex::new(MutexedAshExecution {
				frame: Some(frame),
				wakers: SmallVec::new(),
			}),
		}
	}

	pub unsafe fn new_no_frame(resource: AshExecutionResource, bindless: Bindless<Ash>) -> Self {
		Self {
			bindless,
			resource,
			completed: AtomicBool::new(false),
			mutex: Mutex::new(MutexedAshExecution {
				frame: None,
				wakers: SmallVec::new(),
			}),
		}
	}

	pub fn completed(&self) -> bool {
		self.completed.load(Relaxed)
	}

	fn check_completion(&self, device: &Device) -> bool {
		let value = unsafe { device.get_semaphore_counter_value(self.resource.semaphore).unwrap() };
		if value == self.resource.timeline_value {
			let wakers = {
				let mut guard = self.mutex.lock();
				// must be set while holding `wakers` to prevent races
				self.completed.store(true, Relaxed);
				// frame has finished, drop frame to start resource reclamation
				drop(guard.frame.take());
				mem::replace(&mut guard.wakers, SmallVec::new())
			};
			for x in wakers {
				x.wake();
			}
			true
		} else {
			false
		}
	}

	fn poll(&self, cx: &mut Context<'_>) -> Poll<()> {
		// fast fail
		if self.completed.load(Relaxed) {
			Poll::Ready(())
		} else {
			let mut guard = self.mutex.lock();
			// consistent check
			if self.completed.load(Relaxed) {
				Poll::Ready(())
			} else {
				guard.wakers.push(cx.waker().clone());
				Poll::Pending
			}
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

impl Drop for AshExecution {
	fn drop(&mut self) {
		let bindless = self.bindless();
		bindless
			.execution_manager
			.push_to_free_pool(bindless, self.resource.clone())
	}
}

pub struct AshExecutionManager {
	bindless: WeakBindless<Ash>,
	free_pool: SegQueue<AshExecutionResource>,
	submit_for_waiting: SegQueue<Arc<AshExecution>>,
	wait_thread: Mutex<(Option<thread::ThreadId>, Option<thread::JoinHandle<()>>)>,
	wait_thread_shutdown: AtomicBool,
	wait_thread_notify_semaphore: ash::vk::Semaphore,
	wait_thread_notify_timeline_value_send: Mutex<u64>,
	wait_thread_notify_timeline_value_receive: AtomicU64,
}

pub const ASH_WAIT_SEMAPHORE_THREAD_NAME: &str = "AshWaitSemaphoreThread";

impl AshExecutionManager {
	pub fn new(bindless: &WeakBindless<Ash>, create_info: &AshCreateInfo) -> VkResult<Self> {
		let initial_value = 0;
		Ok(Self {
			bindless: bindless.clone(),
			free_pool: SegQueue::new(),
			submit_for_waiting: SegQueue::new(),
			wait_thread: Mutex::new((None, None)),
			wait_thread_shutdown: AtomicBool::new(false),
			wait_thread_notify_semaphore: create_timeline_semaphore(&create_info.device, initial_value)?,
			wait_thread_notify_timeline_value_send: Mutex::new(initial_value),
			wait_thread_notify_timeline_value_receive: AtomicU64::new(initial_value),
		})
	}

	pub fn bindless(&self) -> Bindless<Ash> {
		self.bindless.upgrade().expect("bindless was freed")
	}

	pub fn new_execution(&self) -> VkResult<Arc<AshExecution>> {
		let bindless = self.bindless();
		Ok(Arc::new(AshExecution::new(
			self.pop_free_pool(&bindless)?,
			bindless.frame(),
		)))
	}

	pub unsafe fn new_execution_no_frame(&self) -> VkResult<Arc<AshExecution>> {
		unsafe {
			let bindless = self.bindless();
			Ok(Arc::new(AshExecution::new_no_frame(
				self.pop_free_pool(&bindless)?,
				bindless,
			)))
		}
	}

	fn pop_free_pool(&self, bindless: &Bindless<Ash>) -> VkResult<AshExecutionResource> {
		Ok(match self.free_pool.pop() {
			None => AshExecutionResource::new(&bindless.device)?,
			Some(e) => e,
		})
	}

	fn push_to_free_pool(&self, bindless: &Bindless<Ash>, mut resource: AshExecutionResource) {
		resource.reset(&bindless.device);
		self.free_pool.push(resource);
	}

	/// # Safety
	/// must only submit an execution acquired from [`Self::new_execution`] exactly once
	pub unsafe fn submit_for_waiting(&self, execution: Arc<AshExecution>) -> VkResult<()> {
		self.assert_not_in_shutdown();
		self.submit_for_waiting.push(execution);
		self.notify_wait_semaphore_thread()?;
		Ok(())
	}

	pub fn notify_wait_semaphore_thread(&self) -> VkResult<()> {
		let mut guard = self.wait_thread_notify_timeline_value_send.lock();
		// no need to send more notifies when one notify is already pending
		if self.wait_thread_notify_timeline_value_receive.load(Relaxed) == *guard {
			*guard += 1;
			unsafe {
				self.bindless().device.signal_semaphore(
					&SemaphoreSignalInfo::default()
						.semaphore(self.wait_thread_notify_semaphore)
						.value(*guard),
				)?;
			}
		}
		Ok(())
	}

	unsafe fn wait_semaphore_thread_main(bindless: Bindless<Ash>) {
		unsafe {
			let execution_manager = &bindless.execution_manager;
			let device = &bindless.device;
			let initial_capacity = 64;
			let mut pending = Vec::with_capacity(initial_capacity);
			let mut semaphores = Vec::with_capacity(initial_capacity);
			let mut values = Vec::with_capacity(initial_capacity);
			let mut notify_timeline_value;
			loop {
				// update our notify_timeline_value
				notify_timeline_value = device
					.get_semaphore_counter_value(execution_manager.wait_thread_notify_semaphore)
					.unwrap();
				execution_manager
					.wait_thread_notify_timeline_value_receive
					.store(notify_timeline_value, Relaxed);

				// add new semaphores
				// MUST happen after timeline_value_receive was set
				while let Some(e) = execution_manager.submit_for_waiting.pop() {
					if !e.check_completion(device) {
						pending.push(e);
					}
				}

				// check each semaphore for completion
				pending.retain(|e| !e.check_completion(device));
				if !pending.is_empty() {
					// wait for any semaphore to complete
					assert!(semaphores.is_empty() && values.is_empty());
					semaphores.extend(
						pending
							.iter()
							.map(|e| e.resource.semaphore)
							.chain([execution_manager.wait_thread_notify_semaphore]),
					);
					values.extend(
						pending
							.iter()
							.map(|e| e.resource.timeline_value)
							.chain([notify_timeline_value + 1]),
					);
					let result = device.wait_semaphores(
						&SemaphoreWaitInfo::default()
							.flags(SemaphoreWaitFlags::ANY)
							.semaphores(&semaphores)
							.values(&values),
						!0,
					);
					semaphores.clear();
					values.clear();
					match result {
						Ok(_) | Err(ash::vk::Result::TIMEOUT) => (),
						Err(e) => panic!("{:?}", e),
					}
				} else {
					if execution_manager.wait_thread_shutdown.load(Relaxed) {
						assert_eq!(pending.len(), 0);
						assert_eq!(execution_manager.submit_for_waiting.len(), 0);
						break;
					}
					// wait for notify semaphore only
					// with timeout to back off from the bindless Arc
					let result = device.wait_semaphores(
						&SemaphoreWaitInfo::default()
							.semaphores(&[execution_manager.wait_thread_notify_semaphore])
							.values(&[notify_timeline_value + 1]),
						!0,
					);
					match result {
						Ok(_) | Err(ash::vk::Result::TIMEOUT) => (),
						Err(e) => panic!("{:?}", e),
					}
				}
			}
		}
	}

	pub fn assert_not_in_shutdown(&self) {
		if self.wait_thread_shutdown.load(Relaxed) {
			panic!("in shutdown")
		}
	}

	pub fn start_wait_semaphore_thread(&self, bindless: &Bindless<Ash>) {
		let mut guard = self.wait_thread.lock();
		if guard.0.is_none() {
			self.assert_not_in_shutdown();
			let bindless = bindless.clone();
			let join_handle = thread::Builder::new()
				.name(ASH_WAIT_SEMAPHORE_THREAD_NAME.to_string())
				.spawn(|| unsafe { Self::wait_semaphore_thread_main(bindless) })
				.unwrap();
			*guard = (Some(join_handle.thread().id()), Some(join_handle));
		}
	}

	pub fn graceful_shutdown(&self) -> VkResult<()> {
		let mut guard = self.wait_thread.lock();
		if let Some(join_handle) = guard.1.take() {
			if join_handle.thread().id() == thread::current().id() {
				panic!(
					"graceful_shutdown() must not be called in {}",
					ASH_WAIT_SEMAPHORE_THREAD_NAME
				);
			}
			self.wait_thread_shutdown.store(true, Relaxed);
			self.notify_wait_semaphore_thread()?;
			join_handle.join().unwrap();
		}
		Ok(())
	}

	pub unsafe fn destroy(&mut self, device: &Device) {
		let guard = self.wait_thread.lock();
		if let Some(join) = guard.0 {
			if guard.1.as_ref().is_some() {
				panic!("Bindless dropped without graceful shutdown");
			}
			if join == thread::current().id() {
				panic!("destroy() must not be called in {}", ASH_WAIT_SEMAPHORE_THREAD_NAME);
			}
		}
		unsafe {
			device.destroy_semaphore(self.wait_thread_notify_semaphore, None);
			while let Some(resource) = self.free_pool.pop() {
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
	fn new_completed() -> Self {
		Self { execution: None }
	}

	fn completed(&self) -> bool {
		match self.upgrade_ash_resource() {
			None => true,
			Some(e) => e.completed(),
		}
	}
}

impl Future for AshPendingExecution {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		if let Some(execution) = self.upgrade_ash_resource() {
			execution.poll(cx)
		} else {
			Poll::Ready(())
		}
	}
}
