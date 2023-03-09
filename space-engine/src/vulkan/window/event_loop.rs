use std::mem::replace;
use std::process::exit;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::mpsc::TryRecvError::Disconnected;
use std::thread::Builder;

use parking_lot::Mutex;
use winit::event::Event;
use winit::event_loop::{EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

type Message = Box<dyn FnOnce(&EventLoopWindowTarget<()>) + Send + 'static>;

/// using a Mutex as Sender is not Sync. Optimally one would clone it once for each sender, but that's probably more effort than using a plain Mutex.
pub static SENDER: Mutex<Option<(Sender<Message>, EventLoopProxy<()>)>> = Mutex::new(None);

struct DropSender {}

impl Drop for DropSender {
	fn drop(&mut self) {
		let mut guard = SENDER.lock();
		let (_, notify) = replace(&mut *guard, None).unwrap();
		notify.send_event(()).unwrap();
	}
}

pub fn run_on_event_loop<F>(f: F)
	where
		F: FnOnce(&EventLoopWindowTarget<()>) + Send + 'static,
{
	let guard = SENDER.lock();
	let (sender, notify) = guard.as_ref().expect("No EventLoop present!");
	sender.send(Box::new(f)).unwrap();
	notify.send_event(()).unwrap();
}

/// needs to be called from main thread, as EventLoop requires to be used on it to be portable between platforms
pub fn event_loop_init<F>(make_event_loop: bool, f: F) -> !
	where
		F: FnOnce(Receiver<Event<()>>) + Send + 'static
{
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
		.name(String::from("Init"))
		.spawn(move || {
			let _drop_sender = DropSender {};
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
							Ok(c) => { c(b) }
							Err(e) => {
								if let Disconnected = e {
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
