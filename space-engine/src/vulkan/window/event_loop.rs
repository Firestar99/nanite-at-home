use std::cell::{Cell, UnsafeCell};
use std::future::Future;
use std::hint::spin_loop;
use std::mem::{MaybeUninit, replace};
use std::pin::Pin;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::task::{Context, Poll, Waker};
use std::thread::yield_now;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;
use winit::event::Event;
use winit::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};

use crate::reinit::{global_need_init, NeedGuard, Reinit, Target};
use crate::reinit_no_restart_map;
use crate::vulkan::window::event_loop::TaskState::*;

trait RunOnEventLoop: Send + Sync {
	fn run(self: Arc<Self>, event_loop: &EventLoopWindowTarget<()>);
}

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

impl<R, F> RunOnEventLoop for TaskInner<R, F>
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


// Messages sent to main loop
enum Message {
	InitEventLoop,
	RunOnEventLoop(Arc<dyn RunOnEventLoop>),
}

#[allow(clippy::type_complexity)]
static SENDER: Mutex<Option<(Sender<Message>, Option<EventLoopProxy<()>>)>> = Mutex::new(None);

fn send(msg: Message) {
	let guard = SENDER.lock();
	let (sender, notify) = guard.as_ref().expect("EventLoop was not initialized!");
	sender.send(msg).unwrap();
	if let Some(notify) = notify {
		notify.send_event(()).unwrap();
	}
}

pub fn event_loop_init(target: &'static Reinit<impl Target>) -> !
{
	// plain setup
	let (tx, exec_rx) = channel();
	{
		let mut guard = SENDER.lock();
		*guard = Some((tx, None));
	}

	// need target
	// SAFETY: this method is exactly made for this case, and should not be called from anywhere else
	// fixme what to do with the need?
	let need = unsafe { global_need_init(target) };
	*ROOT_NEED.lock() = Some(Box::new(need));

	// plain loop without EventLoop
	loop {
		match exec_rx.recv() {
			Ok(msg) => {
				match msg {
					Message::InitEventLoop => {
						break;
					}
					Message::RunOnEventLoop(_) => {
						panic!("EventLoop is not yet initialized!");
					}
				}
			}
			Err(_) => {
				// is always a disconnect
				exit(0);
			}
		};
	}

	// EventLoop setup
	println!("[Info] Main: transitioning to Queue with EventLoop");
	let event_loop = EventLoop::new();
	{
		let mut guard = SENDER.lock();
		let proxy = event_loop.create_proxy();
		// there may be Messages remaining on the queue which need handling
		proxy.send_event(()).unwrap();
		guard.as_mut().unwrap().1 = Some(proxy);
	}

	// EventLoop loop
	event_loop.run(move |event, b, control_flow| {
		match event {
			Event::UserEvent(_) => {
				loop {
					match exec_rx.try_recv() {
						Ok(msg) => {
							match msg {
								Message::InitEventLoop => { panic!("already inited EventLoop!") }
								Message::RunOnEventLoop(run) => { run.run(b) }
							}
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
				// fixme
				drop(event);
				// if let Some(event) = event.to_static() {
				// 	let _ = event_tx.send(event);
				// }
			}
		}
	});
}

pub(crate) fn last_reinit_dropped() {
	let mut guard = SENDER.lock();
	let (sender, notify) = replace(&mut *guard, None).expect("EventLoop was not initialized!");
	drop(sender);
	if let Some(notify) = notify {
		notify.send_event(()).unwrap();
	}
}


// stop
// FIXME: idea how to handle this better
// need system is great for eg. a client starting up an internal server, or a connection to a server, etc.
// How to manage the Lifetime of Client, the root Target of the Application?
// Idea: allow a Target to manage it's own lifetime using their own Instance of NeedGuard
// allows ReinitNoRestart to stay permanently initialized
// allows client to un-need itself to shut down
// Even if a NeedGuard may be dropped immediately, it still needs to construct the Reinit as it may need itself
// Questions:
// Maybe separate these "root"-Targets from normal Targets using a different trait?
// Maybe get rid of current Target (allow every Reinit to be need()) and make it the new root-Target?
// May need a new type of reinit! macro? NO, would suck cause it'll need normal, map and future variant again.
// Maybe couple construction of the self-need onto impl Restart<T: Target>, as we're passing that already?
trait NeedGuardTrait: Send + Sync {}

impl<T: Target> NeedGuardTrait for NeedGuard<T> {}

static ROOT_NEED: Mutex<Option<Box<dyn NeedGuardTrait>>> = Mutex::new(None);

pub fn stop() {
	loop {
		if let Some(need) = ROOT_NEED.lock().take() {
			drop(need);
			return;
		}
		yield_now();
	}
}


// EventLoopAccess
#[derive(Copy, Clone)]
pub struct EventLoopAccess(EventLoopAccessSecret);

// prevent external construction using this secret
#[derive(Copy, Clone)]
struct EventLoopAccessSecret;

reinit_no_restart_map!(pub EVENT_LOOP_ACCESS: EventLoopAccess = {
	send(Message::InitEventLoop);
	EventLoopAccess(EventLoopAccessSecret)
});

impl EventLoopAccess {
	pub fn spawn<R, F>(&self, f: F) -> impl Future<Output=R>
		where
			R: Send + 'static,
			F: FnOnce(&EventLoopWindowTarget<()>) -> R + Send + 'static,
	{
		let task = Arc::new(TaskInner::new(f));
		send(Message::RunOnEventLoop(task.clone()));
		Task(task)
	}
}
