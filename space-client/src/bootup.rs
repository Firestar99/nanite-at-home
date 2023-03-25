use std::sync::Arc;

use clap::Parser;
use vulkano::device::Device;
use vulkano::instance::Instance;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::Surface;
use vulkano_win::create_surface_from_winit;
use winit::window::WindowBuilder;

use space_engine::{reinit, reinit_future, reinit_map, reinit_no_restart};
use space_engine::vulkan::init::{init, Init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::vulkan::window::event_loop::{EVENT_LOOP_ACCESS, EventLoopAccess};
use space_engine::vulkan::window::swapchain::{Swapchain, SwapchainState};
use space_engine::vulkan::window::window_plugin::WindowPlugin;
use space_engine::vulkan::window::window_ref::WindowRef;

use crate::APPLICATION_CONFIG;
use crate::cli_args::Cli;
use crate::vulkan::{Queues, SpaceQueueAllocator};

reinit_no_restart!(pub WINDOW_SYSTEM: bool = true);
reinit_no_restart!(pub CLI: Cli = Cli::parse());
reinit_map!(pub RENDERDOC_ENABLE: bool = (CLI: Cli) => |cli, _| cli.renderdoc);
reinit_map!(pub VALIDATION_LAYER: bool = (CLI: Cli) => |cli, _| cli.validation_layer);
reinit!(pub VULKAN_INIT: Init<Queues> = (VALIDATION_LAYER: bool, RENDERDOC_ENABLE: bool, WINDOW_SYSTEM: bool) =>
	|validation_layer, renderdoc_enable, window_system, _| {
		let mut plugins: Vec<&mut dyn Plugin> = vec![];

		let mut standard_validation_plugin = StandardValidationLayerPlugin;
		if *validation_layer {
			plugins.push(&mut standard_validation_plugin);
		}
		let mut renderdoc_plugin = RenderdocLayerPlugin;
		if *renderdoc_enable {
			plugins.push(&mut renderdoc_plugin);
		}
		let mut window_plugin = WindowPlugin;
		if *window_system {
			plugins.push(&mut window_plugin);
		}

		let init = init(APPLICATION_CONFIG, plugins, SpaceQueueAllocator::new());
		println!("{}", init.device.physical_device().properties().device_name);
		init
});
reinit_map!(pub INSTANCE: Arc<Instance> = (VULKAN_INIT: Init<Queues>) => |init, _| init.instance.clone());
reinit_map!(pub DEVICE: Arc<Device> = (VULKAN_INIT: Init<Queues>) => |init, _| init.device.clone());
reinit!(pub GLOBAL_ALLOCATOR: StandardMemoryAllocator = (DEVICE: Arc<Device>) => |device, _| {
	StandardMemoryAllocator::new_default((*device).clone())
});

// TODO WindowBuilder is not Send, needs separate config type
// reinit!(WINDOW_CONFIG: Mutex<WindowBuilder> = () => |_| Mutex::new(WindowBuilder::new()));
reinit_future!(pub WINDOW: WindowRef = (EVENT_LOOP_ACCESS: EventLoopAccess) => |event_loop, _| {
	event_loop.spawn(move |event_loop| WindowRef::new(WindowBuilder::new().build(event_loop).unwrap()))
});
reinit_future!(pub SURFACE: Arc<Surface> = (EVENT_LOOP_ACCESS: EventLoopAccess, WINDOW: WindowRef, INSTANCE: Arc<Instance>) => |event_loop, window, instance, _| {
	event_loop.spawn(move |event_loop| create_surface_from_winit(window.get_arc(event_loop).clone(), (*instance).clone()).unwrap())
});
reinit!(SWAPCHAIN_STATE: SwapchainState = SwapchainState::default());
reinit_future!(pub SWAPCHAIN: Swapchain = (DEVICE: Arc<Device>, EVENT_LOOP_ACCESS: EventLoopAccess, WINDOW: WindowRef, SURFACE: Arc<Surface>, SWAPCHAIN_STATE: SwapchainState) =>
	|device, event_loop, window, surface, state, restart|event_loop.spawn(move |event_loop| {
		let size = window.get(event_loop).inner_size().into();
		Swapchain::new(device, size, surface, state, restart)
	})
);
