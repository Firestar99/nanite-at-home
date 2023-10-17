use std::cell::{Cell, RefCell, UnsafeCell};
use std::future::Future;
use std::hint::spin_loop;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::task::{Context, Poll, Waker};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;
use static_assertions::{assert_impl_all, assert_not_impl_all};
use winit::event::Event;
use winit::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};

use crate::vulkan::window::event_loop::TaskState::*;

// EventLoop Task
trait EventLoopTaskTrait: Send + Sync {
	fn run(&self, event_loop: &EventLoopWindowTarget<()>);
}

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive)]
enum TaskState {
	WakerSubmitted,
	WakerSubmitting,
	Running,
	Finished,
	ResultTaken,
}

struct EventLoopTaskInner<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{
	state: AtomicU8,
	// has to be an Option tracking it's own existence, as it may be alive or dead while Running, Waker* and is only definitively dead in Finished and ResultToken
	// also does not need synchronization as it's only written in new(), read only from main thread in run() and dropped as exclusive &mut
	func: Cell<Option<F>>,
	result: UnsafeCell<MaybeUninit<R>>,
	waker: UnsafeCell<MaybeUninit<Waker>>,
}

unsafe impl<R, F> Sync for EventLoopTaskInner<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{}

impl<R, F> EventLoopTaskTrait for EventLoopTaskInner<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{
	fn run(&self, event_loop: &EventLoopWindowTarget<()>) {
		let func = self.func.replace(None).expect("Task ran twice?");
		let result = func(event_loop);
		// SAFETY: as long as state != Finished we are the only ones who have access to self.result, and this is only called by the main thread
		unsafe { &mut *self.result.get() }.write(result);

		let mut state_old = self.state.load(Relaxed);
		loop {
			state_old = match TaskState::from_u8(state_old).unwrap() {
				WakerSubmitted => {
					// AcqRel instead of just Release so we can read Waker
					match self.state.compare_exchange_weak(WakerSubmitted as u8, Finished as u8, AcqRel, Relaxed) {
						Ok(_) => {
							// SAFETY: WakerSubmitted means a Waker is present that must be read, awoken and dropped
							unsafe { (*self.waker.get()).assume_init_read() }.wake();
							break;
						}
						Err(e) => e,
					}
				}
				WakerSubmitting => {
					// wait for Waker to be written to self.waker
					spin_loop();
					self.state.load(Relaxed)
				}
				Running => {
					match self.state.compare_exchange_weak(Running as u8, Finished as u8, Release, Relaxed) {
						Ok(_) => break,
						Err(e) => e,
					}
				}
				Finished => unreachable!(),
				ResultTaken => unreachable!(),
			}
		}
	}
}

impl<R, F> EventLoopTaskInner<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{
	fn new(func: F) -> EventLoopTaskInner<R, F> {
		EventLoopTaskInner {
			state: AtomicU8::new(Running as u8),
			func: Cell::new(Some(func)),
			result: UnsafeCell::new(MaybeUninit::uninit()),
			waker: UnsafeCell::new(MaybeUninit::uninit()),
		}
	}

	fn poll(&self, cx: &mut Context<'_>) -> Poll<R> {
		let mut state_old = self.state.load(Relaxed);
		loop {
			state_old = match TaskState::from_u8(state_old).unwrap() {
				WakerSubmitted | WakerSubmitting => unreachable!("poll called with waker already present"),
				Running => {
					match self.state.compare_exchange_weak(Running as u8, WakerSubmitting as u8, Relaxed, Relaxed) {
						Ok(_) => {
							// SAFETY: by setting state to WakerSubmitting we effectively locked self.waker for ourselves
							unsafe { &mut *self.waker.get() }.write(cx.waker().clone());
							match self.state.compare_exchange(WakerSubmitting as u8, WakerSubmitted as u8, Release, Relaxed) {
								Ok(_) => return Poll::Pending,
								Err(_) => unreachable!(),
							}
						}
						Err(e) => e,
					}
				}
				Finished => {
					match self.state.compare_exchange_weak(Finished as u8, ResultTaken as u8, Acquire, Relaxed) {
						Ok(_) => {
							// SAFETY: Finished indicates that result must be present
							return Poll::Ready(unsafe { (*self.result.get()).assume_init_read() });
						}
						Err(e) => e
					}
				}
				ResultTaken => unreachable!("poll called with result already being retrieved"),
			}
		}
	}
}

impl<R, F> Drop for EventLoopTaskInner<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{
	fn drop(&mut self) {
		match TaskState::from_u8(self.state.load(Relaxed)).unwrap() {
			WakerSubmitted => {
				// SAFETY: WakerSubmitted means that this Future never finished and thus never consumed Waker
				unsafe { self.waker.get_mut().assume_init_drop() }
			}
			WakerSubmitting => unreachable!(),
			Running => {}
			Finished => {
				// SAFETY: Finished indicates that result must be present and has not yet been consumed
				unsafe { self.result.get_mut().assume_init_drop() }
			}
			ResultTaken => {}
		}
	}
}

#[derive(Clone)]
struct EventLoopTask<R, F>(Arc<EventLoopTaskInner<R, F>>)
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static;

impl<R, F> Future for EventLoopTask<R, F>
	where
		F: FnOnce(&EventLoopWindowTarget<()>) -> R,
		F: Send + 'static,
		R: Send + 'static,
{
	type Output = R;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.0.poll(cx)
	}
}


// EventLoop execution
static NOTIFY_CREATED: AtomicBool = AtomicBool::new(false);
static NOTIFY: Mutex<Option<EventLoopProxy<()>>> = Mutex::new(None);

#[derive(Clone)]
pub struct EventLoopExecutor {
	sender: Option<Sender<Arc<dyn EventLoopTaskTrait>>>,
	notify: RefCell<Option<EventLoopProxy<()>>>,
}

assert_impl_all!(EventLoopExecutor: Send);
assert_not_impl_all!(EventLoopExecutor: Sync);

impl EventLoopExecutor {
	fn new(sender: Sender<Arc<dyn EventLoopTaskTrait>>) -> Self {
		Self {
			sender: Some(sender),
			notify: RefCell::new(None),
		}
	}

	pub fn spawn<R, F>(&self, f: F) -> impl Future<Output=R>
		where
			F: FnOnce(&EventLoopWindowTarget<()>) -> R,
			F: Send + 'static,
			R: Send + 'static,
	{
		let task = EventLoopTask(Arc::new(EventLoopTaskInner::new(f)));
		self.send(task.0.clone());
		task
	}

	fn send(&self, message: Arc<dyn EventLoopTaskTrait>) {
		self.sender.as_ref().unwrap().send(message).unwrap();
		self.wake();
	}

	fn wake(&self) {
		let mut notify = self.notify.borrow_mut();
		match notify.as_ref() {
			None => {
				if NOTIFY_CREATED.load(Relaxed) {
					let n = NOTIFY.lock().as_ref().unwrap().clone();
					n.send_event(()).unwrap();
					*notify = Some(n);
				}
			}
			Some(notify) => notify.send_event(()).unwrap(),
		}
	}
}

impl Drop for EventLoopExecutor {
	fn drop(&mut self) {
		drop(self.sender.take());
		self.wake();
	}
}

pub fn event_loop_init<F>(launch: F) -> !
	where
		F: FnOnce(EventLoopExecutor, Receiver<Event<'static, ()>>)
{
	// plain setup
	let (exec_tx, exec_rx) = channel();
	let (event_tx, event_rx) = channel();
	launch(EventLoopExecutor::new(exec_tx), event_rx);

	// plain loop without EventLoop
	let event_loop;
	{
		let forward_msg;
		loop {
			match exec_rx.recv() {
				Ok(msg) => {
					forward_msg = msg;
					break;
				}
				Err(_) => {
					// fail is always a disconnect
					exit(0);
				}
			};
		}

		// EventLoop setup
		// FIXME replace with log
		println!("[Info] Main: transitioning to Queue with EventLoop");
		event_loop = EventLoop::new();
		{
			let notify = event_loop.create_proxy();
			// there may be Messages remaining on the queue which need handling
			notify.send_event(()).unwrap();
			*NOTIFY.lock() = Some(notify);
			NOTIFY_CREATED.store(true, Release);
		}
		forward_msg.run(&event_loop);
	}

	// EventLoop loop
	event_loop.run(move |event, b, control_flow| {
		match event {
			Event::UserEvent(_) => {
				loop {
					match exec_rx.try_recv() {
						Ok(msg) => {
							msg.run(&b)
						}
						Err(e) => {
							if matches!(e, TryRecvError::Disconnected) {
								// received disconnect from last_reinit_dropped()
								control_flow.set_exit();
							}
							break;
						}
					}
				}
			}
			event => {
				if let Some(event) = event.to_static() {
					// ignore that channel may have closed
					let _ = event_tx.send(event);
				}
			}
		}
	});
}
