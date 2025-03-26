use crate::app_focus::AppFocus;
use crate::debug_settings_selector::DebugSettingsSelector;
use crate::delta_time::DeltaTimer;
use crate::fps_camera_controller::FpsCameraController;
use crate::fps_ui::FpsUi;
use crate::lod_selector::LodSelector;
use crate::nanite_error_selector::NaniteErrorSelector;
use crate::scene_selector::SceneSelector;
use crate::sun_controller::SunController;
use ash::vk::{PhysicalDeviceMeshShaderFeaturesEXT, ShaderStageFlags};
use egui::{Context, Pos2, RichText, Ui};
use glam::{UVec3, Vec3Swizzles};
use rust_gpu_bindless::descriptor::{BindlessImageUsage, BindlessInstance, DescriptorCounts, ImageDescExt};
use rust_gpu_bindless::pipeline::{ColorAttachment, LoadOp, MutImageAccessExt, Present};
use rust_gpu_bindless::platform::ash::{
	ash_init_single_graphics_queue_with_push_next, AshSingleGraphicsQueueCreateInfo, Debuggers,
};
use rust_gpu_bindless_egui::renderer::{EguiRenderPipeline, EguiRenderer, EguiRenderingOptions};
use rust_gpu_bindless_egui::winit_integration::EguiWinitContext;
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
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowAttributes;

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
			let window = e.create_window(
				WindowAttributes::default()
					.with_inner_size(PhysicalSize::new(1920, 1080))
					.with_title("Nanite at home"),
			)?;
			let extensions = ash_enumerate_required_extensions(e.display_handle()?.as_raw())?;
			Ok::<_, anyhow::Error>((WindowRef::new(Arc::new(window)), extensions))
		})
		.await?;

	let bindless = unsafe {
		BindlessInstance::new(
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
		AshSwapchain::new(&bindless, &event_loop, window.clone(), move |surface, _| {
			AshSwapchainParams::automatic_best(
				&bindless2,
				surface,
				BindlessImageUsage::STORAGE | BindlessImageUsage::COLOR_ATTACHMENT,
				SwapchainImageFormatPreference::UNORM,
			)
		})
	}
	.await?;

	// renderer
	let output_format = swapchain.params().format;
	let render_pipeline_main = RenderPipelineMain::new(
		&bindless,
		output_format,
		MESHLET_GROUP_CAPACITY,
		MESHLET_INSTANCE_CAPACITY,
	)?;
	let mut renderer_main = render_pipeline_main.new_renderer()?;

	let egui_renderer = EguiRenderer::new(bindless.clone());
	let egui_render_pipeline = EguiRenderPipeline::new(egui_renderer.clone(), Some(output_format), None);
	let mut egui_ctx = {
		let renderer = egui_renderer.clone();
		let window = window.clone();
		event_loop
			.spawn(move |e| EguiWinitContext::new(renderer, Context::default(), e, window.get(e).clone()))
			.await
	};

	// model loading
	let mut scene_selector = SceneSelector::new(bindless.clone(), crate::sample_scenes::sample_scenes());

	// main loop
	let mut camera_controls = FpsCameraController::new();
	let mut debug_settings_selector = DebugSettingsSelector::new();
	let mut lod_selector = LodSelector::new();
	let mut nanite_error_selector = NaniteErrorSelector::new();
	let mut app_focus = AppFocus::new(event_loop.clone(), window.clone());
	let mut last_frame = DeltaTimer::default();
	let mut sun_controller = SunController::new();
	let mut fps_ui = FpsUi::new();
	'outer: loop {
		profiling::finish_frame!();
		profiling::scope!("frame");

		// event handling
		for event in inputs.try_iter() {
			swapchain.handle_input(&event);

			if !app_focus.handle_input(&event) {
				if !egui_ctx.on_event(&event).map_or(false, |e| e.consumed) {
					camera_controls.handle_input(&event, app_focus.game_focused);
					debug_settings_selector.handle_input(&event);
				}
			}
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				break 'outer;
			}
		}

		let scene = scene_selector.get_or_load_scene().await?.clone();

		// renderer
		profiling::scope!("render");
		let output_image = swapchain.acquire_image(None).await?;
		let frame_data = {
			let delta_time = last_frame.next();
			fps_ui.update(delta_time);

			let out_extent = UVec3::from(output_image.extent()).xy();
			let fov_y = 90.;
			let camera = Camera::new_perspective_rh_y_flip(
				out_extent,
				fov_y / 360. * 2. * PI,
				0.01,
				1000.,
				AffineTransform::new(camera_controls.update(delta_time)),
			);

			let (sun, ambient_light) = sun_controller.eval_sun(delta_time);
			FrameData {
				camera,
				debug_settings: debug_settings_selector.debug_settings.into(),
				debug_mix: debug_settings_selector.debug_mix_adjusted(),
				debug_lod_level: lod_selector.lod_selection(),
				sun,
				ambient_light,
				nanite: nanite_error_selector.nanite,
			}
		};

		let egui_output = egui_ctx.run(|ctx| {
			egui::Window::new("Nanite at home")
				.fixed_pos(Pos2::new(0., 0.))
				.hscroll(true)
				.show(&ctx, |ui| {
					let space = 6.;
					controls_ui(ui);
					ui.add_space(space);
					scene_selector.ui(ui);
					ui.add_space(space);
					debug_settings_selector.ui(ui);
					ui.add_space(space);
					lod_selector.ui(ui);
					ui.add_space(space);
					nanite_error_selector.ui(ui);
					ui.add_space(space);
					sun_controller.ui(ui);
					ui.add_space(space);
				});
			fps_ui.ui(ctx);
		})?;

		let output_image = bindless.execute(|cmd| {
			let mut output_image = output_image.access_dont_care(&cmd)?;
			if let Err(e) = renderer_main.new_frame(cmd, frame_data, &scene, &mut output_image) {
				return Ok(Err(e));
			}
			let mut output_image = output_image.transition::<ColorAttachment>()?;
			egui_output
				.draw(
					&egui_render_pipeline,
					cmd,
					Some(&mut output_image),
					None,
					EguiRenderingOptions {
						image_rt_load_op: LoadOp::Load,
						..Default::default()
					},
				)
				.unwrap();
			Ok(Ok(output_image.transition::<Present>()?.into_desc()))
		})??;

		swapchain.present_image(output_image)?;
	}
	Ok(())
}

fn controls_ui(ui: &mut Ui) {
	egui::CollapsingHeader::new(RichText::new("Controls:").strong())
		.default_open(true)
		.show(ui, |ui| {
			egui::Grid::new("controls").show(ui, |ui| {
				ui.label("Tab");
				ui.label("switch focus to game / ui");
				ui.end_row();

				ui.label("WASD");
				ui.label("Move the Camera");
				ui.end_row();

				ui.label("Scroll wheel");
				ui.label("Adjust camera speed");
				ui.end_row();

				ui.label("Home");
				ui.label("Reset Camera");
				ui.end_row();
			});
		});
}
