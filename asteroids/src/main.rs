use asteroids::main_loop::run;
use async_std::task::block_on;
use space_engine::event_loop_init;

fn main() {
	#[cfg(feature = "profile-with-puffin")]
	let _puffin_server = {
		profiling::puffin::set_scopes_on(true);
		let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
		puffin_http::Server::new(&server_addr).unwrap()
	};

	event_loop_init(|event_loop, input| block_on(run(event_loop, input)));
}
