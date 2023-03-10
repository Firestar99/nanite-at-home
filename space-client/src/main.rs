use std::sync::Arc;
use std::thread::current;
use std::time::Duration;

use async_std::task::{block_on, sleep};
use futures::FutureExt;
use vulkano::swapchain::Surface;

use space_client::bootup::SURFACE;
use space_engine::reinit;
use space_engine::reinit::Target;
use space_engine::reinit::State::Initialized;
use space_engine::vulkan::window::event_loop::{event_loop_init, run_on_event_loop};

struct Main {}

impl Target for Main {}

reinit!(MAIN: Main = (SURFACE: Arc<Surface>) => |_, _| Main {});

fn main() {
	event_loop_init(true, |_rx| {
		let _need = MAIN.need();
		MAIN.assert_state(Initialized);

		let event_loop = run_on_event_loop(|_| {
			assert_eq!(current().name().unwrap(), "main");
			"sent from main"
		});
		block_on(event_loop.then(|s| async move {
			println!("written in {}: {}", current().name().unwrap(), s);

			println!("waiting 5s until exit");
			sleep(Duration::from_secs(5)).await;
		}));

		println!("exiting...");
	})
}
