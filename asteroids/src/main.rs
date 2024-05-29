use asteroids::main_loop::run;
use futures::executor::block_on;
use space_engine::event_loop_init;

fn main() {
	event_loop_init(|event_loop, input| block_on(run(event_loop, input)));
}
