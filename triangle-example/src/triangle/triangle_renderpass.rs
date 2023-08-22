#![cfg(not(target_arch = "spirv"))]

use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;

use vulkano::device::Device;
use vulkano::image::Image;
use vulkano::image::view::ImageView;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::swapchain::Swapchain;

#[derive(Clone, Debug)]
pub struct TriangleRenderpass {
	renderpass: Arc<RenderPass>,
}

#[repr(usize)]
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum TriangleRenderpassSubpass {
	MAIN,
}

impl TriangleRenderpass {
	pub fn new(device: &Arc<Device>, swapchain: &Arc<Swapchain>) -> TriangleRenderpass {
		let renderpass = vulkano::single_pass_renderpass!(
			device.clone(),
			attachments: {
				color: {
					format: swapchain.image_format(),
					samples: 1,
					load_op: Clear,
					store_op: Store,
				}
			},
			pass: {
				color: [color],
				depth_stencil: {}
			}
		).unwrap();
		TriangleRenderpass { renderpass }
	}

	pub fn subpass(&self, subpass: TriangleRenderpassSubpass) -> Subpass {
		Subpass::from(self.renderpass.clone(), subpass as u32).unwrap()
	}
}

impl Deref for TriangleRenderpass {
	type Target = Arc<RenderPass>;

	fn deref(&self) -> &Self::Target {
		&self.renderpass
	}
}

#[derive(Clone, Debug)]
pub struct TriangleFramebuffer {
	renderpass: TriangleRenderpass,
	framebuffer: Vec<Arc<Framebuffer>>,
}

impl TriangleFramebuffer {
	pub fn new<'a>(renderpass: &TriangleRenderpass, images: impl Iterator<Item=&'a Arc<Image>>) -> Self {
		Self {
			renderpass: renderpass.clone(),
			framebuffer: images.map(|i| {
				let view = ImageView::new_default(i.clone()).unwrap();
				Framebuffer::new(renderpass.renderpass.clone(), FramebufferCreateInfo {
					attachments: vec![view],
					..Default::default()
				}).unwrap()
			}).collect(),
		}
	}

	pub fn renderpass(&self) -> &TriangleRenderpass {
		&self.renderpass
	}

	pub fn framebuffer(&self, index: u32) -> Option<&Arc<Framebuffer>> {
		self.framebuffer.get(index as usize)
	}

	pub fn framebuffers(&self) -> impl Iterator<Item=&Arc<Framebuffer>> {
		self.framebuffer.iter()
	}
}
