use egui::{Pos2, RawInput};
use rust_gpu_bindless::generic::descriptor::{Bindless, BindlessImageUsage, DescriptorCounts, ImageDescExt};
use rust_gpu_bindless::generic::pipeline::{ClearValue, ColorAttachment, LoadOp, MutImageAccessExt, Present};
use rust_gpu_bindless::generic::platform::ash::Debuggers;
use rust_gpu_bindless::generic::platform::ash::{
	ash_init_single_graphics_queue, Ash, AshSingleGraphicsQueueCreateInfo,
};
use rust_gpu_bindless_egui::renderer::{EguiRenderer, EguiRenderingOptions};
use rust_gpu_bindless_winit::ash::{
	ash_enumerate_required_extensions, AshSwapchain, AshSwapchainParams, SwapchainImageFormatPreference,
};
use rust_gpu_bindless_winit::event_loop::{event_loop_init, EventLoopExecutor};
use rust_gpu_bindless_winit::window_ref::WindowRef;
use std::sync::mpsc::Receiver;
use winit::event::{Event, WindowEvent};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowAttributes;

pub fn main() {
	event_loop_init(|event_loop, events| async {
		main_loop(event_loop, events).await.unwrap();
	});
}

pub fn debugger() -> Debuggers {
	Debuggers::Validation
}

pub async fn main_loop(event_loop: EventLoopExecutor, events: Receiver<Event<()>>) -> anyhow::Result<()> {
	if matches!(debugger(), Debuggers::RenderDoc) {
		// renderdoc does not yet support wayland
		std::env::remove_var("WAYLAND_DISPLAY");
		std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
	}

	let (window, window_extensions) = event_loop
		.spawn(|e| {
			let window = e.create_window(WindowAttributes::default().with_title("swapchain triangle"))?;
			let extensions = ash_enumerate_required_extensions(e.display_handle()?.as_raw())?;
			Ok::<_, anyhow::Error>((WindowRef::new(window), extensions))
		})
		.await?;

	let bindless = unsafe {
		Bindless::<Ash>::new(
			ash_init_single_graphics_queue(AshSingleGraphicsQueueCreateInfo {
				instance_extensions: window_extensions,
				extensions: &[ash::khr::swapchain::NAME, ash::ext::mesh_shader::NAME],
				debug: debugger(),
				..AshSingleGraphicsQueueCreateInfo::default()
			})?,
			DescriptorCounts::REASONABLE_DEFAULTS,
		)
	};

	let mut swapchain = unsafe {
		let bindless2 = bindless.clone();
		AshSwapchain::new(&bindless, &event_loop, &window, move |surface, _| {
			AshSwapchainParams::automatic_best(
				&bindless2,
				surface,
				BindlessImageUsage::COLOR_ATTACHMENT,
				SwapchainImageFormatPreference::UNORM,
			)
		})
	}
	.await?;

	let egui_renderer = EguiRenderer::new(bindless.clone());
	let egui_pipeline = egui_renderer.create_pipeline(Some(swapchain.params().format), None);
	let mut egui_ctx = egui_renderer.create_context(egui::Context::default());
	let mut basic_ui = BasicUi::default();

	'outer: loop {
		for event in events.try_iter() {
			swapchain.handle_input(&event);
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				break 'outer;
			}
		}

		let rt = swapchain.acquire_image(None).await?;

		let extent = rt.extent();
		let (egui_render, _full_output) = egui_ctx.run(
			RawInput {
				screen_rect: Some(egui::emath::Rect {
					min: Pos2::ZERO,
					max: Pos2 {
						x: extent.width as f32,
						y: extent.height as f32,
					},
				}),
				..RawInput::default()
			},
			|ctx| basic_ui.ui(ctx),
		)?;

		let rt = bindless.execute(|cmd| {
			let mut rt = rt.access_dont_care::<ColorAttachment>(cmd)?;
			egui_render
				.draw(
					&egui_pipeline,
					cmd,
					Some(&mut rt),
					None,
					EguiRenderingOptions {
						image_rt_load_op: LoadOp::Clear(ClearValue::ColorF([0.; 4])),
						depth_rt_load_op: LoadOp::Load,
					},
				)
				.unwrap();
			Ok(rt.transition::<Present>()?.into_desc())
		})?;
		swapchain.present_image(rt)?;
	}

	Ok(())
}

pub struct BasicUi {
	pub name: String,
	pub age: u32,
}

impl Default for BasicUi {
	fn default() -> Self {
		Self {
			name: "Authur".to_string(),
			age: 42,
		}
	}
}

impl BasicUi {
	pub fn ui(&mut self, ctx: &egui::Context) {
		egui::CentralPanel::default().show(ctx, |ui| {
			ui.add_space(100.0);
			ui.heading("My egui Application");
			ui.horizontal(|ui| {
				let name_label = ui.label("Your name: ");
				ui.text_edit_singleline(&mut self.name).labelled_by(name_label.id);
			});
			ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
			if ui.button("Increment").clicked() {
				self.age += 1;
			}
			ui.label(format!("Hello '{}', age {}", self.name, self.age));
		});
	}
}
