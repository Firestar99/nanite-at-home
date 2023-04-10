#![cfg(not(target_arch = "spirv"))]

use std::sync::Arc;

use vulkano::device::Device;
use vulkano::memory::allocator::StandardMemoryAllocator;

use space_engine::reinit;
use space_engine::space::bootup::{DEVICE, GLOBAL_ALLOCATOR, SWAPCHAIN, VULKAN_INIT};
use space_engine::space::queue_allocation::Queues;
use space_engine::vulkan::init::Init;
use space_engine::vulkan::window::swapchain::Swapchain;

use crate::triangle::triangle_main::TriangleMain;
use crate::triangle::triangle_model::TriangleModel;
use crate::triangle::triangle_pipeline::TrianglePipeline;
use crate::triangle::triangle_renderpass::{TriangleFramebuffer, TriangleRenderpass};

reinit!(pub TRIANGLE_RENDERPASS: TriangleRenderpass = (DEVICE: Arc<Device>, SWAPCHAIN: Swapchain) =>
	|device, swapchain, _| TriangleRenderpass::new(&device, &swapchain)
);
reinit!(pub TRIANGLE_FRAMEBUFFER: TriangleFramebuffer = (TRIANGLE_RENDERPASS: TriangleRenderpass, SWAPCHAIN: Swapchain) =>
	|renderpass, swapchain, _| TriangleFramebuffer::new(&renderpass, swapchain.images())
);
reinit!(pub TRIANGLE_PIPELINE: TrianglePipeline = (DEVICE: Arc<Device>, TRIANGLE_RENDERPASS: TriangleRenderpass) =>
	|device, renderpass, _| TrianglePipeline::new(&device, &renderpass)
);
reinit!(pub TRIANGLE_MODEL: TriangleModel = (GLOBAL_ALLOCATOR: StandardMemoryAllocator) =>
	|allocator, _| TriangleModel::new_basic_model(&*allocator)
);
reinit!(pub TRIANGLE_MAIN: TriangleMain = (VULKAN_INIT: Init<Queues>, SWAPCHAIN: Swapchain, TRIANGLE_FRAMEBUFFER: TriangleFramebuffer, TRIANGLE_PIPELINE: TrianglePipeline, TRIANGLE_MODEL: TriangleModel) =>
	|init, swapchain, framebuffer, pipeline, model, _| TriangleMain::new(init, swapchain, framebuffer, pipeline, model)
);
