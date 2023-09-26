use std::sync::Arc;

use async_global_executor::{spawn, Task};
use spirv_std::glam::{Affine3A, Mat4};
use vulkano::swapchain::SwapchainPresentInfo;
use vulkano::sync::GpuFuture;

use space_engine::CallOnDrop;
use space_engine::reinit::{ReinitRef, Target};
use space_engine::space::queue_allocation::Queues;
use space_engine::space::renderer::lod_obj::opaque_render_task::OpaqueRenderTask;
use space_engine::space::renderer::render_graph::context::RenderContext;
use space_engine::vulkan::init::Init;
use space_engine::vulkan::window::event_loop::stop;
use space_engine::vulkan::window::swapchain::Swapchain;
use space_engine_common::space::renderer::camera::Camera;
use space_engine_common::space::renderer::frame_data::FrameData;

struct Inner {
	init: ReinitRef<Arc<Init<Queues>>>,
	swapchain: ReinitRef<Swapchain>,
}

#[allow(dead_code)]
pub struct MainLoop {
	main: Arc<Inner>,
	worker: Task<()>,
}

impl Target for MainLoop {}

impl MainLoop {
	pub fn new(
		init: ReinitRef<Arc<Init<Queues>>>,
		swapchain: ReinitRef<Swapchain>,
	) -> MainLoop {
		let main = Arc::new(Inner { init, swapchain });
		MainLoop { worker: main.clone().run(), main }
	}
}

impl Inner {
	fn run(self: Arc<Self>) -> Task<()> {
		spawn(async move {
			let _stop = CallOnDrop(stop);

			let (render_context, new_frame) = RenderContext::new((*self.init).clone(), self.swapchain.image_format(), 2);
			let opaque_render_task = OpaqueRenderTask::new(&render_context, render_context.output_format);

			let graphics_main = &self.init.queues.client.graphics_main;
			loop {
				let (swapchain_acquire, output_image) = self.swapchain.acquire_image(None);

				let frame_data = FrameData {
					camera: Camera {
						camera: Affine3A::default(),
						perspective: Mat4::default(),
						perspective_inverse: Mat4::default(),
					},
				};
				new_frame.new_frame(output_image.clone(), frame_data, |frame_context, prev_frame| {
					let prev_frame = prev_frame.join(swapchain_acquire);
					let opaque_future = opaque_render_task.record(&frame_context, prev_frame);
					let present_future = opaque_future.then_swapchain_present(graphics_main.clone(), SwapchainPresentInfo::swapchain_image_index((**self.swapchain).clone(), swapchain_acquire.image_index()));
					present_future.boxed().then_signal_fence_and_flush().unwrap()
				});
			}
		})
	}
}
