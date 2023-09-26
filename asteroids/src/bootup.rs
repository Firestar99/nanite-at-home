use std::sync::Arc;

use space_engine::reinit;
use space_engine::space::bootup::{SWAPCHAIN, VULKAN_INIT};
use space_engine::space::Init;
use space_engine::vulkan::window::swapchain::Swapchain;

use crate::mainloop::MainLoop;

reinit!(pub MAINLOOP: MainLoop = (VULKAN_INIT: Arc<Init>, SWAPCHAIN: Swapchain) => |init, swapchain, _| {
	MainLoop::new(init, swapchain)
});
