use std::sync::Arc;
use std::thread::current;
use std::time::Duration;

use async_global_executor::{spawn, Task};
use async_std::task::sleep;

use space_client::bootup::SWAPCHAIN;
use space_engine::{init, reinit};
use space_engine::reinit::{ReinitRef, Target};
use space_engine::vulkan::window::event_loop::{EVENT_LOOP_ACCESS, EventLoopAccess, stop};
use space_engine::vulkan::window::swapchain::Swapchain;

pub struct Main {
	pub event_loop: ReinitRef<EventLoopAccess>,
	pub swapchain: ReinitRef<Swapchain>,
}

pub struct MainTarget {
	pub main: Arc<Main>,
	pub worker: Task<()>,
}

impl Target for MainTarget {}

reinit!(MAIN: MainTarget = (EVENT_LOOP_ACCESS: EventLoopAccess, SWAPCHAIN: Swapchain) => |event_loop, swapchain, _| {
	let main = Arc::new(Main { event_loop, swapchain });
	MainTarget { worker: main.clone().run(), main }
});

impl Main {
	fn run(self: Arc<Self>) -> Task<()> {
		spawn(async move {
			let from_main = self.event_loop.spawn(|_| {
				assert_eq!(current().name().unwrap(), "main");
				String::from("sent from main")
			}).await;

			println!("written in {}: {}", current().name().unwrap(), from_main);

			let _ = self.swapchain.acquire_image(None).unwrap();

			println!("waiting 5s until exit");
			sleep(Duration::from_millis(5000)).await;

			println!("exiting...");
			stop();
		})
	}
}

fn main() {
	init(&MAIN);
}
