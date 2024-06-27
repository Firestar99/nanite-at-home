use glam::{Mat4, UVec3};
use space_engine::generate_application_config;
use space_engine::space::queue_allocation::SpaceQueueAllocator;
use space_engine::space::renderer::renderer_plugin::RendererPlugin;
use space_engine::space::renderer::renderers::main::{RenderPipelineMain, RendererMain};
use space_engine::space::Init;
use space_engine::vulkan::init::Plugin;
use space_engine::vulkan::plugins::rust_gpu_workaround::RustGpuWorkaround;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::vulkan::plugins::vulkano_bindless::VulkanoBindless;
use space_engine::vulkan::window::event_loop::EventLoopExecutor;
use space_engine::vulkan::window::swapchain::Swapchain;
use space_engine::vulkan::window::window_plugin::WindowPlugin;
use space_engine::vulkan::window::window_ref::WindowRef;
use space_engine_shader::space::renderer::camera::Camera;
use space_engine_shader::space::renderer::frame_data::FrameData;
use std::f32::consts::PI;
use std::num::NonZeroUsize;
use std::sync::mpsc::Receiver;
use vulkano::shader::ShaderStages;
use vulkano::sync::GpuFuture;
use vulkano_bindless::descriptor::descriptor_counts::DescriptorCounts;
use winit::event::{Event, WindowEvent};
use winit::window::{CursorGrabMode, WindowBuilder};

use crate::delta_time::DeltaTimeTimer;
use crate::fps_camera_controller::FpsCameraController;
use crate::sample_scene::load_scene;

pub enum Debugger {
	None,
	Validation,
	RenderDoc,
}

const DEBUGGER: Debugger = Debugger::RenderDoc;

pub async fn run(event_loop: EventLoopExecutor, inputs: Receiver<Event<()>>) {
	if matches!(DEBUGGER, Debugger::RenderDoc) {
		// renderdoc does not yet support wayland
		std::env::remove_var("WAYLAND_DISPLAY");
		std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
	}
	std::env::set_var(
		"SMOL_THREADS",
		std::thread::available_parallelism()
			.unwrap_or(NonZeroUsize::new(1).unwrap())
			.get()
			.to_string(),
	);

	let init;
	{
		let window_plugin = WindowPlugin::new(&event_loop).await;
		let mut vec: Vec<&dyn Plugin> = vec![&RendererPlugin, &RustGpuWorkaround, &VulkanoBindless, &window_plugin];
		if matches!(DEBUGGER, Debugger::Validation) {
			vec.push(&StandardValidationLayerPlugin);
		}

		let stages = ShaderStages::TASK
			| ShaderStages::MESH
			| ShaderStages::VERTEX
			| ShaderStages::FRAGMENT
			| ShaderStages::COMPUTE;
		init = Init::new(
			generate_application_config!(),
			&vec,
			SpaceQueueAllocator::new(),
			stages,
			DescriptorCounts::reasonable_defaults,
		)
		.await;
	}
	let graphics_main = &init.queues.client.graphics_main;

	// window
	let window = event_loop
		.spawn(move |event_loop| {
			WindowRef::new({
				let window = WindowBuilder::new().build(event_loop).unwrap();
				window.set_cursor_grab(CursorGrabMode::Locked).ok();
				window.set_cursor_visible(false);
				window
			})
		})
		.await;
	let (swapchain, mut swapchain_controller) = Swapchain::new(graphics_main.clone(), event_loop, window.clone()).await;

	// renderer
	let render_pipeline_main = RenderPipelineMain::new(&init, swapchain.format());
	let mut renderer_main: Option<RendererMain> = None;

	// model loading
	let scenes = load_scene(&init).await;
	render_pipeline_main.meshlet_task.scenes.lock().extend(scenes);

	// main loop
	let mut camera_controls = FpsCameraController::new();
	let mut last_frame = DeltaTimeTimer::default();
	'outer: loop {
		// event handling
		for event in inputs.try_iter() {
			swapchain_controller.handle_input(&event);
			camera_controls.handle_input(&event);
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				break 'outer;
			}
		}

		// renderer
		let (swapchain_acquire, acquired_image) = swapchain_controller.acquire_image(None).await;
		if renderer_main.as_ref().map_or(true, |renderer| {
			renderer.image_supported(acquired_image.image_view()).is_err()
		}) {
			// drop then recreate to better recycle memory
			drop(renderer_main.take());
			renderer_main = Some(render_pipeline_main.new_renderer(acquired_image.image_view().image().extent(), 2));
		}

		// frame data
		let delta_time = last_frame.next();
		let image = UVec3::from_array(acquired_image.image_view().image().extent());
		let frame_data = FrameData {
			camera: Camera::new(
				Mat4::perspective_rh(90. / 360. * 2. * PI, image.x as f32 / image.y as f32, 0.1, 1000.),
				camera_controls.update(delta_time),
			),
		};

		renderer_main.as_mut().unwrap().new_frame(
			frame_data,
			acquired_image.image_view().clone(),
			|frame, prev_frame_future, main_frame| {
				let future = main_frame.record(swapchain_acquire.join(prev_frame_future));
				acquired_image.present(frame.frame, future)
			},
		);
	}

	init.pipeline_cache.write().await.ok();
}
