use std::sync::Arc;
use std::thread::current;
use std::time::Duration;

use async_global_executor::{spawn, Task};
use async_std::task::sleep;
use vulkano::swapchain::Surface;

use space_client::bootup::SURFACE;
use space_engine::reinit;
use space_engine::reinit::{ReinitRef, Target};
use space_engine::vulkan::window::event_loop::{EVENT_LOOP_ACCESS, event_loop_init, EventLoopAccess, stop};

pub struct Main {
	pub event_loop: ReinitRef<EventLoopAccess>,
	pub surface: ReinitRef<Arc<Surface>>,
}

pub struct MainTarget {
	pub main: Arc<Main>,
	pub worker: Task<()>,
}

impl Target for MainTarget {}

reinit!(MAIN: MainTarget = (EVENT_LOOP_ACCESS: EventLoopAccess, SURFACE: Arc<Surface>) => |event_loop, surface, _| {
	let main = Arc::new(Main { event_loop, surface });
	MainTarget { worker: main.run(), main }
});

impl Main {
	fn run(&self) -> Task<()> {
		let event_loop = *self.event_loop;
		spawn(async move {
			let from_main = event_loop.spawn(|_| {
				assert_eq!(current().name().unwrap(), "main");
				String::from("sent from main")
			}).await;

			println!("written in {}: {}", current().name().unwrap(), from_main);

			println!("waiting 5s until exit");
			sleep(Duration::from_millis(500)).await;

			println!("exiting...");
			stop();
		})
	}
}

fn main() {
	event_loop_init(&MAIN);
}
