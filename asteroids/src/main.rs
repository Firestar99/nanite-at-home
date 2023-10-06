use std::sync::mpsc::Receiver;
use std::thread;

use futures::executor::block_on;
use spirv_std::glam::{Affine3A, Mat4};
use vulkano::swapchain::Surface;
use vulkano::sync::GpuFuture;
use winit::event::Event;
use winit::window::WindowBuilder;

use space_engine::{event_loop_init, generate_application_config};
use space_engine::space::Init;
use space_engine::space::queue_allocation::SpaceQueueAllocator;
use space_engine::space::renderer::lod_obj::opaque_render_task::OpaqueRenderTask;
use space_engine::space::renderer::render_graph::context::RenderContext;
use space_engine::vulkan::init::Plugin;
use space_engine::vulkan::plugins::dynamic_rendering::DynamicRendering;
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::rust_gpu_workaround::RustGpuWorkaround;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::vulkan::window::event_loop::EventLoopExecutor;
use space_engine::vulkan::window::swapchain::Swapchain;
use space_engine::vulkan::window::window_plugin::WindowPlugin;
use space_engine::vulkan::window::window_ref::WindowRef;
use space_engine_common::space::renderer::camera::Camera;
use space_engine_common::space::renderer::frame_data::FrameData;


fn main() {
	event_loop_init(|event_loop, input| {
		thread::spawn(move || block_on(run(event_loop, input)));
	});
}

async fn run(event_loop: EventLoopExecutor, _input: Receiver<Event<'static, ()>>) {
	let layer_renderdoc = false;
	let layer_validation = true;

	let init;
	{
		let window_plugin = WindowPlugin::new(&event_loop).await;
		let mut vec: Vec<&dyn Plugin> = vec![
			&DynamicRendering,
			&RustGpuWorkaround,
			&window_plugin,
		];
		if layer_renderdoc {
			vec.push(&RenderdocLayerPlugin);
		}
		if layer_validation {
			vec.push(&StandardValidationLayerPlugin);
		}

		init = Init::new(generate_application_config!(), &vec, SpaceQueueAllocator::new());
	}

	// FIXME dimensions is constant and never re-queried
	let (_window, surface, dimensions) = {
		let init = init.clone();
		event_loop.spawn(move |event_loop| {
			let window = WindowRef::new(WindowBuilder::new().build(event_loop).unwrap());
			let surface = Surface::from_window(init.instance().clone(), window.get_arc(event_loop).clone()).unwrap();
			let dimensions = window.get(event_loop).inner_size().into();
			(window, surface, dimensions)
		}).await
	};
	let (swapchain, mut swapchain_controller) = Swapchain::new(init.device.clone(), surface.clone(), dimensions);
	let (render_context, mut new_frame) = RenderContext::new(init.clone(), swapchain.format(), 2);
	let opaque_render_task = OpaqueRenderTask::new(&render_context, render_context.output_format);

	let graphics_main = &init.queues.client.graphics_main;
	loop {
		let (swapchain_acquire, output_image, present_info) = swapchain_controller.acquire_image(dimensions, None);

		let frame_data = FrameData {
			camera: Camera {
				transform: Affine3A::default(),
				perspective: Mat4::default(),
				perspective_inverse: Mat4::default(),
			},
		};

		new_frame.new_frame(output_image.clone(), frame_data, |frame_context| {
			let opaque_future = opaque_render_task.record(&frame_context, swapchain_acquire);
			let present_future = opaque_future.then_swapchain_present(graphics_main.clone(), present_info);
			// FIXME presenting may fail with out of date swapchain
			present_future.boxed().then_signal_fence_and_flush().unwrap()
		});
	}
}
