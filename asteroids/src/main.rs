use std::thread;

use futures::executor::block_on;

use asteroids::main_loop::run;
use space_engine::event_loop_init;

fn main() {
	event_loop_init(|event_loop, input| {
		thread::spawn(move || block_on(run(event_loop, input)));
	});
}
