use glam::{Mat4, UVec3};
use space_engine::device::init::Plugin;
use space_engine::device::plugins::rust_gpu_workaround::RustGpuWorkaround;
use space_engine::device::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;
use space_engine::device::plugins::vulkano_bindless::VulkanoBindless;
use space_engine::generate_application_config;
use space_engine::renderer::queue_allocation::SpaceQueueAllocator;
use space_engine::renderer::renderer_plugin::RendererPlugin;
use space_engine::renderer::renderers::main::{RenderPipelineMain, RendererMain};
use space_engine::renderer::Init;
use space_engine::window::event_loop::EventLoopExecutor;
use space_engine::window::swapchain::Swapchain;
use space_engine::window::window_plugin::WindowPlugin;
use space_engine::window::window_ref::WindowRef;
use space_engine_shader::renderer::camera::Camera;
use space_engine_shader::renderer::frame_data::FrameData;
use std::f32::consts::PI;
use std::sync::mpsc::Receiver;
use vulkano::shader::ShaderStages;
use vulkano::sync::GpuFuture;
use vulkano_bindless::descriptor::descriptor_counts::DescriptorCounts;
use winit::event::{Event, WindowEvent};
use winit::window::{CursorGrabMode, WindowBuilder};

use crate::delta_time::DeltaTimeTimer;
use crate::fps_camera_controller::FpsCameraController;
use crate::sample_scene::SceneSelector;

pub enum Debugger {
	None,
	Validation,
	RenderDoc,
}

const DEBUGGER: Debugger = Debugger::None;

pub async fn run(event_loop: EventLoopExecutor, inputs: Receiver<Event<()>>) {
	if matches!(DEBUGGER, Debugger::RenderDoc) {
		// renderdoc does not yet support wayland
		std::env::remove_var("WAYLAND_DISPLAY");
		std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
	}

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
			|phy| {
				DescriptorCounts {
					buffers: 100_000,
					..DescriptorCounts::reasonable_defaults(phy)
				}
				.min(DescriptorCounts::limits(phy))
			},
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
	let scenes = Vec::from([
		models::local::gamescom::bistro::Bistro,
		models::local::gamescom::Sponza::glTF::Sponza,
		models::local::gamescom::San_Miguel::san_miguel,
		models::local::gamescom::rungholt::rungholt,
		models::local::gamescom::lost_empire::lost_empire,
		models::local::gamescom::vokselia_spawn::vokselia_spawn,
		models::local::gamescom::DamagedHelmet::glTF::DamagedHelmet,
		models::Lantern::glTF::Lantern,
		models::local::gamescom::lpshead::head,
		models::local::gamescom::sibenik::sibenik,
	]);
	let mut scene_selector = SceneSelector::new(init.clone(), scenes, |scene| {
		let mut guard = render_pipeline_main.meshlet_task.scenes.lock();
		guard.clear();
		guard.push(scene);
	})
	.await
	.unwrap();

	// main loop
	let mut camera_controls = FpsCameraController::new();
	let mut last_frame = DeltaTimeTimer::default();
	'outer: loop {
		profiling::finish_frame!();
		profiling::scope!("frame");
		// event handling
		for event in inputs.try_iter() {
			swapchain_controller.handle_input(&event);
			camera_controls.handle_input(&event);
			scene_selector.handle_input(&event).await.unwrap();
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
			profiling::scope!("recreate renderer");
			// drop then recreate to better recycle memory
			drop(renderer_main.take());
			renderer_main = Some(render_pipeline_main.new_renderer(acquired_image.image_view().image().extent(), 2));
		}

		// frame data
		profiling::scope!("render");
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

	init.pipeline_cache.write().ok();
}
