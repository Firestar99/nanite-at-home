use std::sync::Arc;

use clap::Parser;
use vulkano::device::Device;
use vulkano::instance::Instance;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::Surface;
use winit::window::WindowBuilder;

use crate::{reinit, reinit_future, reinit_map, reinit_no_restart};
use crate::space::cli_args::Cli;
use crate::space::engine_config::get_config;
use crate::space::Init;
use crate::space::queue_allocation::SpaceQueueAllocator;
use crate::vulkan::init::Plugin;
use crate::vulkan::plugins::default_device_selection_plugin::DefaultDeviceSelectionPlugin;
use crate::vulkan::plugins::dynamic_rendering::DynamicRendering;
use crate::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use crate::vulkan::plugins::rust_gpu_workaround::RustGpuWorkaround;
use crate::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use crate::vulkan::window::event_loop::{EVENT_LOOP_ACCESS, EventLoopAccess};
use crate::vulkan::window::swapchain::{Swapchain, SwapchainState};
use crate::vulkan::window::window_plugin::WindowPlugin;
use crate::vulkan::window::window_ref::WindowRef;

reinit_no_restart!(pub WINDOW_SYSTEM: bool = true);
reinit_no_restart!(pub CLI: Cli = Cli::parse());
reinit_map!(pub RENDERDOC_ENABLE: bool = (CLI: Cli) => |cli, _| cli.renderdoc);
reinit_map!(pub VALIDATION_LAYER: bool = (CLI: Cli) => |cli, _| cli.validation_layer);
reinit_future!(pub VULKAN_INIT: Arc<Init> = (EVENT_LOOP_ACCESS: EventLoopAccess, VALIDATION_LAYER: bool, RENDERDOC_ENABLE: bool, WINDOW_SYSTEM: bool) =>
	|event_loop, validation_layer, renderdoc_enable, window_system, _| { async {
		let mut plugins: Vec<&mut dyn Plugin> = vec![];

		let mut standard_validation_plugin = StandardValidationLayerPlugin;
		if *validation_layer {
			plugins.push(&mut standard_validation_plugin);
		}
		let mut renderdoc_plugin = RenderdocLayerPlugin;
		if *renderdoc_enable {
			plugins.push(&mut renderdoc_plugin);
		}
		let mut dynamic_rendering = DynamicRendering;
		plugins.push(&mut dynamic_rendering);

		// FIXME Window extensions since vulkano 0.34 are derived from EventLoop, but on headless constructing EventLoop will panic due to no window system being found.
		// so we cannot just enable window extensions on "we may need it in the future"
		let mut window_plugin = WindowPlugin::new(*event_loop).await;
		if *window_system {
			plugins.push(&mut window_plugin);
		}
		let mut rust_gpu_workaround = RustGpuWorkaround;
		plugins.push(&mut rust_gpu_workaround);

		let mut b = DefaultDeviceSelectionPlugin;
		plugins.push(&mut b);

		let init = Arc::new(Init::new(get_config().application_config, plugins, SpaceQueueAllocator::new()));
		println!("{}", init.device.physical_device().properties().device_name);
		init
}});
reinit_map!(pub INSTANCE: Arc<Instance> = (VULKAN_INIT: Arc<Init>) => |init, _| init.instance().clone());
reinit_map!(pub DEVICE: Arc<Device> = (VULKAN_INIT: Arc<Init>) => |init, _| init.device.clone());
reinit!(pub GLOBAL_ALLOCATOR: StandardMemoryAllocator = (DEVICE: Arc<Device>) => |device, _| {
	StandardMemoryAllocator::new_default((*device).clone())
});

// TODO WindowBuilder is not Send, needs separate config type
// reinit!(WINDOW_CONFIG: Mutex<WindowBuilder> = () => |_| Mutex::new(WindowBuilder::new()));
reinit_future!(pub WINDOW: WindowRef = (EVENT_LOOP_ACCESS: EventLoopAccess) => |event_loop, _| {
	event_loop.spawn(move |event_loop| WindowRef::new(WindowBuilder::new().build(event_loop).unwrap()))
});
reinit_future!(pub SURFACE: Arc<Surface> = (EVENT_LOOP_ACCESS: EventLoopAccess, WINDOW: WindowRef, INSTANCE: Arc<Instance>) => |event_loop, window, instance, _| {
	event_loop.spawn(move |event_loop| Surface::from_window((*instance).clone(), window.get_arc(event_loop).clone()).unwrap())
});
reinit!(SWAPCHAIN_STATE: SwapchainState = SwapchainState::default());
reinit_future!(pub SWAPCHAIN: Swapchain = (DEVICE: Arc<Device>, EVENT_LOOP_ACCESS: EventLoopAccess, WINDOW: WindowRef, SURFACE: Arc<Surface>, SWAPCHAIN_STATE: SwapchainState) =>
	|device, event_loop, window, surface, state, restart|event_loop.spawn(move |event_loop| {
		let size = window.get(event_loop).inner_size().into();
		Swapchain::new(device, size, surface, state, restart)
	})
);
