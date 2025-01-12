use crate::curser_lock::CursorLock;
use crate::debug_settings_selector::DebugSettingsSelector;
use crate::delta_time::DeltaTimer;
use crate::fps_camera_controller::FpsCameraController;
use crate::lod_selector::LodSelector;
use crate::nanite_error_selector::NaniteErrorSelector;
use crate::sample_scenes::sample_scenes;
use crate::scene_selector::SceneSelector;
use crate::sun_controller::{eval_ambient_light, eval_sun};
use ash::vk::{PhysicalDeviceMeshShaderFeaturesEXT, ShaderStageFlags};
use glam::{UVec3, Vec3Swizzles};
use parking_lot::Mutex;
use rust_gpu_bindless::descriptor::{BindlessImageUsage, DescriptorCounts, ImageDescExt};
use rust_gpu_bindless::generic::descriptor::Bindless;
use rust_gpu_bindless::pipeline::{MutImageAccessExt, Present};
use rust_gpu_bindless::platform::ash::{
	ash_init_single_graphics_queue_with_push_next, Ash, AshSingleGraphicsQueueCreateInfo, Debuggers,
};
use rust_gpu_bindless_winit::ash::{
	ash_enumerate_required_extensions, AshSwapchain, AshSwapchainParams, SwapchainImageFormatPreference,
};
use rust_gpu_bindless_winit::event_loop::EventLoopExecutor;
use rust_gpu_bindless_winit::window_ref::WindowRef;
use space_asset_shader::affine_transform::AffineTransform;
use space_engine::renderer::renderers::main::RenderPipelineMain;
use space_engine_shader::renderer::camera::Camera;
use space_engine_shader::renderer::frame_data::FrameData;
use std::f32::consts::PI;
use std::sync::mpsc::Receiver;
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowBuilder;

const DEBUGGER: Debuggers = Debuggers::None;

/// how many `MeshletInstance`s can be dynamically allocated, 1 << 17 = 131072
/// about double what bistro needs if all meshlets rendered
const MESHLET_INSTANCE_CAPACITY: usize = 1 << 19;

/// how many `MeshletGroupInstance` can be dynamically allocated
const MESHLET_GROUP_CAPACITY: usize = 1 << 19;

pub async fn main_loop(event_loop: EventLoopExecutor, inputs: Receiver<Event<()>>) -> anyhow::Result<()> {
	rayon::ThreadPoolBuilder::new()
		.thread_name(|i| format!("rayon worker {i}"))
		.build_global()?;
	if matches!(DEBUGGER, Debuggers::RenderDoc) {
		// renderdoc does not yet support wayland
		std::env::remove_var("WAYLAND_DISPLAY");
		std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
	}

	let (window, window_extensions) = event_loop
		.spawn(|e| {
			let window = WindowBuilder::new()
				.with_inner_size(PhysicalSize::new(1920, 1080))
				.with_title("Nanite at home")
				.build(e)?;
			let extensions = ash_enumerate_required_extensions(e.display_handle()?.as_raw())?;
			Ok::<_, anyhow::Error>((WindowRef::new(window), extensions))
		})
		.await?;

	let bindless = unsafe {
		Bindless::<Ash>::new(
			ash_init_single_graphics_queue_with_push_next(
				AshSingleGraphicsQueueCreateInfo {
					instance_extensions: window_extensions,
					extensions: &[ash::khr::swapchain::NAME, ash::ext::mesh_shader::NAME],
					shader_stages: ShaderStageFlags::ALL_GRAPHICS
						| ShaderStageFlags::COMPUTE
						| ShaderStageFlags::MESH_EXT,
					debug: DEBUGGER,
					..AshSingleGraphicsQueueCreateInfo::default()
				},
				Some(&mut PhysicalDeviceMeshShaderFeaturesEXT::default().mesh_shader(true)),
			)?,
			DescriptorCounts {
				buffers: 100_000,
				..DescriptorCounts::REASONABLE_DEFAULTS
			},
		)
	};

	let mut swapchain = unsafe {
		let bindless2 = bindless.clone();
		AshSwapchain::new(&bindless, &event_loop, &window, move |surface, _| {
			AshSwapchainParams::automatic_best(
				&bindless2,
				surface,
				BindlessImageUsage::STORAGE,
				SwapchainImageFormatPreference::UNORM,
			)
		})
	}
	.await?;

	// renderer
	let render_pipeline_main = RenderPipelineMain::new(
		&bindless,
		swapchain.params().format,
		MESHLET_GROUP_CAPACITY,
		MESHLET_INSTANCE_CAPACITY,
	)?;
	let mut renderer_main = render_pipeline_main.new_renderer()?;

	// model loading
	let scene = Mutex::new(None);
	let mut scene_selector = SceneSelector::new(bindless.clone(), sample_scenes(), |s| {
		*scene.lock() = Some(s);
	})
	.await?;

	// main loop
	let mut camera_controls = FpsCameraController::new();
	let mut debug_settings_selector = DebugSettingsSelector::new();
	let mut lod_selector = LodSelector::new();
	let mut nanite_error_selector = NaniteErrorSelector::new();
	let mut cursor_lock = CursorLock::new(event_loop.clone(), window.clone());
	let mut last_frame = DeltaTimer::default();
	'outer: loop {
		profiling::finish_frame!();
		profiling::scope!("frame");

		// event handling
		for event in inputs.try_iter() {
			swapchain.handle_input(&event);
			camera_controls.handle_input(&event);
			debug_settings_selector.handle_input(&event);
			scene_selector.handle_input(&event).await?;
			lod_selector.handle_input(&event);
			nanite_error_selector.handle_input(&event);
			cursor_lock.handle_input(&event);
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				break 'outer;
			}
		}

		// renderer
		profiling::scope!("render");
		let output_image = swapchain.acquire_image(None).await?;
		let frame_data = {
			let delta_time = last_frame.next();

			let out_extent = UVec3::from(output_image.extent()).xy();
			let fov_y = 90.;
			let camera = Camera::new_perspective_rh_y_flip(
				out_extent,
				fov_y / 360. * 2. * PI,
				0.01,
				1000.,
				AffineTransform::new(camera_controls.update(delta_time)),
			);

			let sun = eval_sun(delta_time);
			let ambient_light = eval_ambient_light(sun);

			FrameData {
				camera,
				nanite_error_threshold: nanite_error_selector.error,
				debug_settings: debug_settings_selector.get().into(),
				debug_lod_level: lod_selector.lod_level,
				sun,
				ambient_light,
			}
		};

		let output_image = bindless.execute(|cmd| {
			let mut output_image = output_image.access_dont_care(&cmd)?;
			if let Err(e) = renderer_main.new_frame(cmd, frame_data, &scene.lock().as_ref().unwrap(), &mut output_image)
			{
				return Ok(Err(e));
			}
			Ok(Ok(output_image.transition::<Present>()?.into_desc()))
		})??;

		swapchain.present_image(output_image)?;
	}
	Ok(())
}
