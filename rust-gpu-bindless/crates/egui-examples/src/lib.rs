use rust_gpu_bindless_core::descriptor::{BindlessImageUsage, BindlessInstance, DescriptorCounts};
use rust_gpu_bindless_core::pipeline::{ClearValue, ColorAttachment, LoadOp, MutImageAccessExt, Present};
use rust_gpu_bindless_core::platform::ash::Debuggers;
use rust_gpu_bindless_core::platform::ash::{Ash, AshSingleGraphicsQueueCreateInfo, ash_init_single_graphics_queue};
use rust_gpu_bindless_egui::renderer::{EguiRenderPipeline, EguiRenderer, EguiRenderingOptions};
use rust_gpu_bindless_egui::winit_integration::EguiWinitContext;
use rust_gpu_bindless_winit::ash::{
	AshSwapchain, AshSwapchainParams, SwapchainImageFormatPreference, ash_enumerate_required_extensions,
};
use rust_gpu_bindless_winit::event_loop::{EventLoopExecutor, event_loop_init};
use rust_gpu_bindless_winit::window_ref::WindowRef;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use winit::event::{Event, WindowEvent};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowAttributes;

pub fn run(run_ui: impl FnMut(&egui::Context) + Send + 'static) {
	event_loop_init(|event_loop, events| async move {
		main_loop(event_loop, events, run_ui).await.unwrap();
	});
}

pub fn debugger() -> Debuggers {
	Debuggers::Validation
}

pub async fn main_loop(
	event_loop: EventLoopExecutor,
	events: Receiver<Event<()>>,
	mut run_ui: impl FnMut(&egui::Context),
) -> anyhow::Result<()> {
	if matches!(debugger(), Debuggers::RenderDoc) {
		unsafe {
			// renderdoc does not yet support wayland
			std::env::remove_var("WAYLAND_DISPLAY");
			std::env::set_var("ENABLE_VULKAN_RENDERDOC_CAPTURE", "1");
		}
	}

	let (window, window_extensions) = event_loop
		.spawn(|e| {
			let window = e.create_window(WindowAttributes::default().with_title("swapchain triangle"))?;
			let extensions = ash_enumerate_required_extensions(e.display_handle()?.as_raw())?;
			Ok::<_, anyhow::Error>((WindowRef::new(Arc::new(window)), extensions))
		})
		.await?;

	let bindless = unsafe {
		BindlessInstance::<Ash>::new(
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
		AshSwapchain::new(&bindless, &event_loop, window.clone(), move |surface, _| {
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
	let egui_pipeline = EguiRenderPipeline::new(egui_renderer.clone(), Some(swapchain.params().format), None);
	let mut egui_ctx = event_loop
		.spawn(move |e| {
			EguiWinitContext::new(
				egui_renderer.clone(),
				egui::Context::default(),
				e,
				window.get(e).clone(),
			)
		})
		.await;

	'outer: loop {
		profiling::scope!("frame");
		for event in events.try_iter() {
			swapchain.handle_input(&event);
			egui_ctx.on_event(&event);
			if let Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} = &event
			{
				break 'outer;
			}
		}

		let egui_render = egui_ctx.run(|ctx| run_ui(ctx))?;

		let rt = swapchain.acquire_image(None).await?;
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
						..EguiRenderingOptions::default()
					},
				)
				.unwrap();
			Ok(rt.transition::<Present>()?.into_desc())
		})?;
		swapchain.present_image(rt)?;

		profiling::finish_frame!();
	}

	Ok(())
}
