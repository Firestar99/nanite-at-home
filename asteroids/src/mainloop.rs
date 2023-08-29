use std::sync::Arc;

use async_global_executor::{spawn, Task};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::swapchain::SwapchainPresentInfo;
use vulkano::sync::{GpuFuture, now};

use space_engine::CallOnDrop;
use space_engine::reinit::{ReinitRef, Target};
use space_engine::space::queue_allocation::Queues;
use space_engine::space::renderer::lodobj;
use space_engine::vulkan::init::Init;
use space_engine::vulkan::window::event_loop::stop;
use space_engine::vulkan::window::swapchain::Swapchain;

struct Inner {
	init: ReinitRef<Init<Queues>>,
	swapchain: ReinitRef<Swapchain>,
	lodobj_rendertask: ReinitRef<lodobj::render_task::RenderTask>,
}

#[allow(dead_code)]
pub struct MainLoop {
	main: Arc<Inner>,
	worker: Task<()>,
}

impl Target for MainLoop {}

impl MainLoop {
	pub fn new(
		init: ReinitRef<Init<Queues>>,
		swapchain: ReinitRef<Swapchain>,
		lodobj_rendertask: ReinitRef<lodobj::render_task::RenderTask>,
	) -> MainLoop {
		let main = Arc::new(Inner { init, swapchain, lodobj_rendertask });
		MainLoop { worker: main.clone().run(), main }
	}
}

impl Inner {
	fn run(self: Arc<Self>) -> Task<()> {
		spawn(async move {
			let _stop = CallOnDrop(stop);

			let graphics_main = &self.init.queues.client.graphics_main;
			let allocator = StandardCommandBufferAllocator::new(self.init.device.clone(), Default::default());

			let mut prev_frame_fence: Box<dyn GpuFuture> = now(self.init.device.clone()).boxed();
			loop {
				let swapchain_acquire = self.swapchain.acquire_image(None);
				let fif_index = swapchain_acquire.image_index();

				let mut cmd = AutoCommandBufferBuilder::primary(&allocator, self.init.queues.client.graphics_main.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
				self.lodobj_rendertask.record(fif_index as usize, &mut cmd);
				let draw_cmd = cmd.build().unwrap();

				prev_frame_fence.cleanup_finished();
				prev_frame_fence = swapchain_acquire
					.join(prev_frame_fence)
					.then_execute(graphics_main.clone(), draw_cmd).unwrap()
					.then_swapchain_present(graphics_main.clone(), SwapchainPresentInfo::swapchain_image_index((*self.swapchain).clone(), fif_index))
					.then_signal_fence_and_flush().unwrap()
					.boxed();
			}
		})
	}
}
