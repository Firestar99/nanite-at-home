use space_engine::reinit;
use space_engine::space::bootup::{SWAPCHAIN, VULKAN_INIT};
use space_engine::space::Init;
use space_engine::space::renderer::lod_obj::bootup::OPAQUE;
use space_engine::space::renderer::lod_obj::opaque::OpaquePipeline;
use space_engine::space::renderer::lod_obj::render_task::RenderTask;
use space_engine::vulkan::window::swapchain::Swapchain;

use crate::mainloop::MainLoop;

reinit!(pub RENDERTASK: RenderTask = (OPAQUE: OpaquePipeline, SWAPCHAIN: Swapchain) => |opaque, swapchain, _| {
	RenderTask::new(opaque, swapchain.images())
});
reinit!(pub MAINLOOP: MainLoop = (VULKAN_INIT: Init, SWAPCHAIN: Swapchain, RENDERTASK: RenderTask) => |init, swapchain, rendertask, _| {
	MainLoop::new(init, swapchain, rendertask)
});
