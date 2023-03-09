use std::cell::{Cell, UnsafeCell};
use std::future::Future;
use std::hint::spin_loop;
use std::mem::{MaybeUninit, replace};
use std::pin::Pin;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::mpsc::TryRecvError::Disconnected;
use std::task::{Context, Poll, Waker};
use std::thread::Builder;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;
use winit::event::Event;
use winit::event_loop::{EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use crate::CallOnDrop;
use crate::vulkan::window::event_loop::TaskState::*;

// TaskInner
#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive)]
enum TaskState {
	WakerSubmitted,
	WakerSubmitting,
	Running,
	Finished,
	ResultTaken,
}

struct TaskInner<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{
	state: AtomicU8,
	// has to be an Option tracking it's own existence, as it may be alive or dead while Running, Waker* and is only definitively dead in Finished and ResultToken
	// also does not need synchronization as it's only written in new(), read only from main thread in run() and dropped as exclusive &mut
	func: Cell<Option<F>>,
	result: UnsafeCell<MaybeUninit<R>>,
	waker: UnsafeCell<MaybeUninit<Waker>>,
}

unsafe impl<R, F> Sync for TaskInner<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{}

impl<R, F> Runnable for TaskInner<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{
	fn run(self: Arc<Self>, event_loop: &EventLoopWindowTarget<()>) {
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

impl<R, F> TaskInner<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{
	fn new(func: F) -> TaskInner<R, F> {
		TaskInner {
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

impl<R, F> Drop for TaskInner<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
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
struct Task<R, F>(Arc<TaskInner<R, F>>)
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static;

impl<R, F> Future for Task<R, F>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{
	type Output = R;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.0.poll(cx)
	}
}

trait Runnable: Send + Sync {
	fn run(self: Arc<Self>, event_loop: &EventLoopWindowTarget<()>);
}


// executor
/// using a Mutex as Sender is not Sync. Optimally one would clone it once for each sender, but that's probably more effort than using a plain Mutex.
type SenderNotify = (Sender<Arc<dyn Runnable>>, EventLoopProxy<()>);

static SENDER: Mutex<Option<SenderNotify>> = Mutex::new(None);

pub fn run_on_event_loop<R, F>(f: F) -> impl Future<Output=R>
	where
		R: Send + 'static,
		F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
{
	let task = Arc::new(TaskInner::new(f));
	let guard = SENDER.lock();
	let (sender, notify) = guard.as_ref().expect("No EventLoop present!");
	sender.send(task.clone()).unwrap();
	notify.send_event(()).unwrap();
	Task(task)
}

/// needs to be called from main thread, as EventLoop requires to be used on it to be portable between platforms
pub fn event_loop_init<F>(make_event_loop: bool, f: F) -> !
	where
		F: FnOnce(Receiver<Event<()>>) + Send + 'static
{
	// sending events from main to application
	let (tx, rx) = channel();

	let event_loop = if make_event_loop {
		let event_loop = EventLoopBuilder::new().build();
		let (tx, rx) = channel();
		*SENDER.lock() = Some((tx, event_loop.create_proxy()));
		Some((rx, event_loop))
	} else {
		None
	};

	let join_handle = Builder::new()
		.name(String::from("init"))
		.spawn(move || {
			let _drop_sender = CallOnDrop(|| {
				let mut guard = SENDER.lock();
				let (_, notify) = replace(&mut *guard, None).unwrap();
				notify.send_event(()).unwrap();
			});
			f(rx)
		})
		.unwrap();

	if let Some((rx, event_loop)) = event_loop {
		drop(join_handle);
		event_loop.run(move |event, b, control_flow| {
			match event {
				Event::UserEvent(_) => {
					loop {
						match rx.try_recv() {
							Ok(c) => { c.run(b) }
							Err(e) => {
								if matches!(e, Disconnected) {
									control_flow.set_exit();
								}
								break;
							}
						}
					}
				}
				event => {
					if let Some(event) = event.to_static() {
						tx.send(event).ok();
					}
				}
			}
		});
	} else {
		// this code does NOT get run when using EventLoop as EventLoop::run() never returns due to calling exit() directly!
		join_handle
			.join()
			.unwrap();
		exit(0);
	}
}
