use std::sync::Arc;

use clap::Parser;
use futures::executor::block_on;
use vulkano::swapchain::Surface;
use vulkano_win::create_surface_from_winit;
use winit::window::WindowBuilder;

use space_engine::{reinit, reinit_no_restart};
use space_engine::vulkan::init::{init, Init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::vulkan::window::event_loop::run_on_event_loop;
use space_engine::vulkan::window::window_plugin::WindowPlugin;
use space_engine::vulkan::window::window_ref::WindowRef;

use crate::APPLICATION_CONFIG;
use crate::cli_args::Cli;
use crate::vulkan::{Queues, SpaceQueueAllocator};

reinit_no_restart!(pub WINDOW_SYSTEM: bool = true);
reinit_no_restart!(pub CLI: Cli = Cli::parse());
reinit!(pub RENDERDOC_ENABLE: bool = (CLI: Cli) => |cli, _| cli.renderdoc);
reinit!(pub VALIDATION_LAYER: bool = (CLI: Cli) => |cli, _| cli.validation_layer);
reinit!(pub VULKAN_INIT: Init<Queues> = (VALIDATION_LAYER: bool, RENDERDOC_ENABLE: bool, WINDOW_SYSTEM: bool) =>
	|validation_layer, renderdoc_enable, window_system, _| {
		let mut plugins: Vec<&mut dyn Plugin> = vec![];

		let mut standard_validation_plugin = StandardValidationLayerPlugin;
		if **validation_layer {
			plugins.push(&mut standard_validation_plugin);
		}
		let mut renderdoc_plugin = RenderdocLayerPlugin;
		if **renderdoc_enable {
			plugins.push(&mut renderdoc_plugin);
		}
		let mut window_plugin = WindowPlugin;
		if **window_system {
			plugins.push(&mut window_plugin);
		}

		let init = init(APPLICATION_CONFIG, plugins, SpaceQueueAllocator::new());
		println!("{}", init.device.physical_device().properties().device_name);
		init
});

// TODO WindowBuilder is not Send, needs separate config type
// reinit!(WINDOW_CONFIG: Mutex<WindowBuilder> = () => |_| Mutex::new(WindowBuilder::new()));
// FIXME these should not be using block_on() but instead just delay the internal call to constructed()
reinit!(pub WINDOW: WindowRef = () => |_| block_on(run_on_event_loop(|event_loop| WindowRef::new(WindowBuilder::new().build(event_loop).unwrap()))));
reinit!(pub SURFACE: Arc<Surface> = (WINDOW: WindowRef, VULKAN_INIT: Init<Queues>) => |window, init, _| {
	let instance = init.instance.clone();
	let window = (**window).clone();
	block_on(run_on_event_loop(move |event_loop| create_surface_from_winit(window.get_arc(event_loop).clone(), instance).unwrap()))
});
