use std::f32::consts::PI;
use std::sync::mpsc::Receiver;

use glam::{Mat4, UVec3};
use vulkano::sync::GpuFuture;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;

use space_engine::generate_application_config;
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

use crate::delta_time::DeltaTimeTimer;
use crate::fps_camera_controller::FpsCameraController;

pub async fn run(event_loop: EventLoopExecutor, inputs: Receiver<Event<'static, ()>>) {
	let layer_renderdoc = true;
	let layer_validation = false;

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
	let graphics_main = &init.queues.client.graphics_main;

	let window = event_loop.spawn(move |event_loop| {
		WindowRef::new(WindowBuilder::new().build(event_loop).unwrap())
	}).await;
	let (swapchain, mut swapchain_controller) = Swapchain::new(graphics_main.clone(), event_loop, window.clone()).await;
	let (render_context, mut new_frame) = RenderContext::new(init.clone(), swapchain.format(), 2);
	let opaque_render_task = OpaqueRenderTask::new(&render_context, render_context.output_format);

	let mut camera_controls = FpsCameraController::new();
	let mut last_frame = DeltaTimeTimer::new();
	'outer: loop {
		for event in inputs.try_iter() {
			camera_controls.handle_input(&event);
			match &event {
				Event::WindowEvent {
					event: WindowEvent::CloseRequested,
					..
				} => {
					break 'outer;
				}

				_ => ()
			}
		}

		let (swapchain_acquire, acquired_image) = swapchain_controller.acquire_image(None).await;

		let delta_time = last_frame.next();
		let image = UVec3::from_array(acquired_image.image_view().image().extent());
		let frame_data = FrameData {
			camera: Camera::new(
				Mat4::perspective_rh(90. / 360. * 2. * PI, image.x as f32 / image.y as f32, 0.001, 100.),
				camera_controls.update(delta_time),
			),
		};

		new_frame.new_frame(acquired_image.image_view().clone(), frame_data, |frame_context| {
			let opaque_future = opaque_render_task.record(&frame_context, swapchain_acquire);
			let present_future = acquired_image.present(opaque_future)?;
			Some(present_future.boxed().then_signal_fence_and_flush().unwrap())
		});
	}
}
