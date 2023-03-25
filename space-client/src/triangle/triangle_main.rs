use std::mem::forget;
use std::sync::Arc;
use std::time::Instant;

use async_global_executor::{spawn, Task};
use vulkano::buffer::TypedBufferAccess;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::format::ClearValue;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::swapchain::SwapchainPresentInfo;
use vulkano::sync::{FenceSignalFuture, GpuFuture};

use space_engine::CallOnDrop;
use space_engine::reinit::{ReinitRef, Target};
use space_engine::vulkan::init::Init;
use space_engine::vulkan::window::event_loop::stop;
use space_engine::vulkan::window::swapchain::Swapchain;

use crate::triangle::triangle_model::TriangleModel;
use crate::triangle::triangle_pipeline::TrianglePipeline;
use crate::triangle::triangle_renderpass::TriangleFramebuffer;
use crate::vulkan::Queues;

struct Inner {
	init: ReinitRef<Init<Queues>>,
	swapchain: ReinitRef<Swapchain>,
	framebuffer: ReinitRef<TriangleFramebuffer>,
	pipeline: ReinitRef<TrianglePipeline>,
	model: ReinitRef<TriangleModel>,
}

pub struct TriangleMain {
	main: Arc<Inner>,
	worker: Task<()>,
}

impl Target for TriangleMain {}

impl TriangleMain {
	pub fn new(
		init: ReinitRef<Init<Queues>>,
		swapchain: ReinitRef<Swapchain>,
		framebuffer: ReinitRef<TriangleFramebuffer>,
		pipeline: ReinitRef<TrianglePipeline>,
		model: ReinitRef<TriangleModel>,
	) -> TriangleMain {
		let main = Arc::new(Inner { init, swapchain, framebuffer, pipeline, model });
		TriangleMain { worker: main.clone().run(), main }
	}
}

impl Inner {
	fn run(self: Arc<Self>) -> Task<()> {
		spawn(async move {
			let _stop = CallOnDrop(stop);

			let graphics_main = &self.init.queues.client.graphics_main;
			let viewport = Viewport {
				origin: [0f32, 0f32],
				dimensions: self.swapchain.image_extent().map(|x| x as f32),
				depth_range: 0f32..1f32,
			};
			let allocator = StandardCommandBufferAllocator::new(self.init.device.clone(), Default::default());
			let start = Instant::now();

			let mut prev_frame_fence: Option<FenceSignalFuture<Box<dyn GpuFuture>>> = None;
			loop {
				let swapchain_acquire = match self.swapchain.acquire_image(None) {
					Ok(e) => e,
					Err(_) => {
						forget(_stop);
						break;
					}
				};
				let swapchain_image_index = swapchain_acquire.image_index();

				let mut draw_cmd = AutoCommandBufferBuilder::primary(&allocator, self.init.queues.client.graphics_main.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
				draw_cmd
					.begin_render_pass(RenderPassBeginInfo {
						clear_values: vec![Some(ClearValue::Float([0.0f32; 4]))],
						..RenderPassBeginInfo::framebuffer(self.framebuffer.framebuffer(swapchain_image_index).unwrap().clone())
					}, SubpassContents::Inline).unwrap()
					.set_viewport(0, [viewport.clone()])
					.bind_pipeline_graphics((*self.pipeline).clone())
					.bind_vertex_buffers(0, (**self.model).clone())
					.draw(self.model.0.len() as u32, 1, 0, 0).unwrap()
					.end_render_pass().unwrap();
				let draw_cmd = draw_cmd.build().unwrap();

				if let Some(fence) = prev_frame_fence {
					fence.wait(None).unwrap();
				}
				self.model.update(start.elapsed().as_secs_f32() / 6.);

				prev_frame_fence = Some(swapchain_acquire
					.then_execute(graphics_main.clone(), draw_cmd).unwrap()
					.then_swapchain_present(graphics_main.clone(), SwapchainPresentInfo::swapchain_image_index((*self.swapchain).clone(), swapchain_image_index))
					.boxed()
					.then_signal_fence_and_flush().unwrap()
				);
			}
		})
	}
}
