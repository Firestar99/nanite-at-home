use meshlet_renderer::main_loop::main_loop;
use rust_gpu_bindless_winit::event_loop::event_loop_init;

fn main() {
	#[cfg(feature = "profile-with-puffin")]
	let _puffin_server = {
		profiling::puffin::set_scopes_on(true);
		let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
		puffin_http::Server::new(&server_addr).unwrap()
	};

	event_loop_init(|event_loop, input| async { main_loop(event_loop, input).await.unwrap() });
}
