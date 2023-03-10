use std::thread::current;

use async_std::task::block_on;
use futures::FutureExt;

use space_client::bootup::VULKAN_INIT;
use space_client::vulkan::Queues;
use space_engine::reinit;
use space_engine::reinit::{ReinitRef, Target};
use space_engine::reinit::State::Initialized;
use space_engine::vulkan::init::Init;
use space_engine::vulkan::window::event_loop::{event_loop_init, run_on_event_loop};

struct Main {
	_init: ReinitRef<Init<Queues>>,
}

impl Target for Main {}

reinit!(MAIN: Main = (VULKAN_INIT: Init<Queues>) => |init, _| Main {_init: init.clone()});

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
		}));

		println!("exiting...");
	})
}
