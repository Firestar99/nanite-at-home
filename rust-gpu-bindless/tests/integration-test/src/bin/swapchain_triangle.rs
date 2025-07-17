use ash::vk::{
	ColorComponentFlags, CullModeFlags, FrontFace, PipelineColorBlendAttachmentState,
	PipelineColorBlendStateCreateInfo, PolygonMode, PrimitiveTopology,
};
use glam::{Vec2, Vec4};
use integration_test::debugger;
use integration_test_shader::color::ColorEnum;
use integration_test_shader::triangle::{Param, Vertex};
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageUsage,
	BindlessInstance, DescriptorCounts, Format, Image2d, MutDesc, MutImage, RCDescExt,
};
use rust_gpu_bindless_core::pipeline::DrawIndirectCommand;
use rust_gpu_bindless_core::pipeline::{
	BindlessGraphicsPipeline, ClearValue, ColorAttachment, GraphicsPipelineCreateInfo, LoadOp, MutImageAccessExt,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineRasterizationStateCreateInfo,
	Present, RenderPassFormat, RenderingAttachment, StoreOp,
};
use rust_gpu_bindless_core::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_core::platform::ash::Debuggers;
use rust_gpu_bindless_core::platform::ash::{Ash, AshSingleGraphicsQueueCreateInfo, ash_init_single_graphics_queue};
use rust_gpu_bindless_winit::ash::{
	AshSwapchain, AshSwapchainParams, SwapchainImageFormatPreference, ash_enumerate_required_extensions,
};
use rust_gpu_bindless_winit::event_loop::{EventLoopExecutor, event_loop_init};
use rust_gpu_bindless_winit::window_ref::WindowRef;
use smallvec::SmallVec;
use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowAttributes;

pub fn main() {
	event_loop_init(|event_loop, events| async {
		main_loop(event_loop, events).await.unwrap();
	});
}

pub async fn main_loop(event_loop: EventLoopExecutor, events: Receiver<Event<()>>) -> anyhow::Result<()> {
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
				extensions: &[ash::khr::swapchain::NAME],
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
				SwapchainImageFormatPreference::SRGB,
			)
		})
	}
	.await?;

	let mut renderer = TriangleRenderer::new(&bindless, swapchain.params().format)?;

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
		let rt = renderer.draw(rt).await?;
		swapchain.present_image(rt)?;
	}

	Ok(())
}

#[derive(Debug, Copy, Clone)]
pub struct TriangleRendererRTFormat {
	rt_format: Format,
}

impl TriangleRendererRTFormat {
	pub fn to_render_pass_format(&self) -> RenderPassFormat {
		RenderPassFormat {
			color_attachments: SmallVec::from_slice(&[self.rt_format]),
			depth_attachment: None,
		}
	}
}

pub struct TriangleRenderer<P: BindlessPipelinePlatform> {
	bindless: Bindless<P>,
	rt_format: TriangleRendererRTFormat,
	pipeline: BindlessGraphicsPipeline<P, Param<'static>>,
	timer: Instant,
}

const VERTEX_CNT: usize = 3;

impl<P: BindlessPipelinePlatform> TriangleRenderer<P> {
	pub fn new(bindless: &Bindless<P>, rt_format: Format) -> anyhow::Result<Self> {
		let rt_format = TriangleRendererRTFormat { rt_format };

		let pipeline = bindless.create_graphics_pipeline(
			&rt_format.to_render_pass_format(),
			&GraphicsPipelineCreateInfo {
				input_assembly_state: PipelineInputAssemblyStateCreateInfo::default()
					.topology(PrimitiveTopology::TRIANGLE_LIST),
				rasterization_state: PipelineRasterizationStateCreateInfo::default()
					.polygon_mode(PolygonMode::FILL)
					.front_face(FrontFace::COUNTER_CLOCKWISE)
					.cull_mode(CullModeFlags::BACK),
				depth_stencil_state: PipelineDepthStencilStateCreateInfo::default(),
				color_blend_state: PipelineColorBlendStateCreateInfo::default().attachments(&[
					PipelineColorBlendAttachmentState::default().color_write_mask(ColorComponentFlags::RGBA),
				]),
			},
			integration_test::shader::triangle::triangle_vertex::new(),
			integration_test::shader::triangle::triangle_fragment::new(),
		)?;

		Ok(Self {
			bindless: bindless.clone(),
			rt_format,
			pipeline,
			timer: Instant::now(),
		})
	}

	pub fn generate_vertices(&self) -> [Vertex; VERTEX_CNT] {
		let time = Instant::now().duration_since(self.timer).as_secs_f32() * 0.3 * 360.;
		let pos = |offset: f32| {
			let offset = (time + offset) * 2. * PI / 360.;
			Vec2::new(f32::sin(offset), f32::cos(offset))
		};
		[
			Vertex::new(pos(0.), Vec4::new(1., 0., 0., 1.)),
			Vertex::new(pos(120.), Vec4::new(0., 1., 0., 1.)),
			Vertex::new(pos(240.), Vec4::new(0., 0., 1., 1.)),
		]
	}

	pub async fn draw(&mut self, rt: MutDesc<P, MutImage<Image2d>>) -> anyhow::Result<MutDesc<P, MutImage<Image2d>>> {
		let vertices = self.bindless.buffer().alloc_shared_from_iter(
			&BindlessBufferCreateInfo {
				name: "vertices",
				usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			self.generate_vertices().into_iter(),
		)?;
		let rt = self.bindless.execute(|cmd| {
			let mut rt = rt.access_dont_care::<ColorAttachment>(cmd)?;
			cmd.begin_rendering(
				self.rt_format.to_render_pass_format(),
				&[RenderingAttachment {
					image: &mut rt,
					load_op: LoadOp::Clear(ClearValue::ColorF(ColorEnum::Black.color().to_array())),
					store_op: StoreOp::Store,
				}],
				None,
				|rp| {
					rp.draw(
						&self.pipeline,
						DrawIndirectCommand {
							vertex_count: VERTEX_CNT as u32,
							instance_count: 1,
							..DrawIndirectCommand::default()
						},
						Param {
							vertices: vertices.to_transient(rp),
						},
					)?;
					Ok(())
				},
			)?;

			Ok(rt.transition::<Present>()?.into_desc())
		})?;
		Ok(rt)
	}
}
