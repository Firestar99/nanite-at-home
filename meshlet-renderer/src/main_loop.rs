use crate::debug_settings_selector::DebugSettingsSelector;
use crate::delta_time::DeltaTimer;
use crate::fps_camera_controller::FpsCameraController;
use crate::sample_scenes::sample_scenes;
use crate::scene_selector::SceneSelector;
use glam::{vec3, vec4, Mat3, Mat4, UVec3, Vec3, Vec3Swizzles};
use space_asset::affine_transform::AffineTransform;
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
use space_engine_shader::material::light::DirectionalLight;
use space_engine_shader::material::radiance::Radiance;
use space_engine_shader::renderer::camera::Camera;
use space_engine_shader::renderer::frame_data::FrameData;
use space_engine_shader::renderer::lighting::sky_shader::preetham_sky;
use std::f32::consts::PI;
use std::sync::mpsc::Receiver;
use vulkano::shader::ShaderStages;
use vulkano::sync::GpuFuture;
use vulkano_bindless::descriptor::descriptor_counts::DescriptorCounts;
use winit::event::{Event, WindowEvent};
use winit::window::{CursorGrabMode, WindowBuilder};

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
	let mut scene_selector = SceneSelector::new(init.clone(), sample_scenes(), |scene| {
		let mut guard = render_pipeline_main.meshlet_task.scenes.lock();
		guard.clear();
		guard.push(scene);
	})
	.await
	.unwrap();

	// main loop
	let mut camera_controls = FpsCameraController::new();
	let mut debug_settings_selector = DebugSettingsSelector::new();
	let mut last_frame = DeltaTimer::default();
	'outer: loop {
		profiling::finish_frame!();
		profiling::scope!("frame");
		// event handling
		for event in inputs.try_iter() {
			swapchain_controller.handle_input(&event);
			camera_controls.handle_input(&event);
			debug_settings_selector.handle_input(&event);
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

		profiling::scope!("render");
		let frame_data = {
			let delta_time = last_frame.next();
			let out_extent = UVec3::from_array(acquired_image.image_view().image().extent());
			let projection = Mat4::perspective_rh(
				90. / 360. * 2. * PI,
				out_extent.x as f32 / out_extent.y as f32,
				0.1,
				1000.,
			) * Mat4::from_cols(
				vec4(1., 0., 0., 0.),
				vec4(0., -1., 0., 0.),
				vec4(0., 0., 1., 0.),
				vec4(0., 0., 0., 1.),
			);

			let sun = {
				const SUN_MAX_ALTITUDE_DEGREE: f32 = 25.;
				const SUN_INCLINATION_START: f32 = 0.;
				const SUN_INCLINATION_SPEED: f32 = 0.05;

				let sun_dir = vec3(0., 1., 0.);
				let inclination = SUN_INCLINATION_START + SUN_INCLINATION_SPEED * delta_time.since_start;
				let sun_dir = Mat3::from_axis_angle(vec3(1., 0., 0.), inclination * 2. * PI) * sun_dir;
				let sun_dir =
					Mat3::from_axis_angle(vec3(0., 0., 1.), f32::to_radians(SUN_MAX_ALTITUDE_DEGREE)) * sun_dir;
				// not strictly necessary, but why not correct some inaccuracy?
				let sun_dir = sun_dir.normalize();

				let color = preetham_sky(sun_dir, sun_dir) / 1_000_000.;
				let color = color.clamp(Vec3::splat(0.), Vec3::splat(1.));
				DirectionalLight {
					direction: sun_dir,
					color: Radiance(color),
				}
			};

			FrameData {
				camera: Camera::new(projection, AffineTransform::new(camera_controls.update(delta_time))),
				debug_settings: debug_settings_selector.get().into(),
				viewport_size: out_extent.xy(),
				sun,
				ambient_light: Radiance(Vec3::splat(0.1)),
			}
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
