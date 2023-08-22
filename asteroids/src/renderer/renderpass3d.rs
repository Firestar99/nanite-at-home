use std::sync::Arc;

use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::ImageLayout;
use vulkano::render_pass::{AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp, RenderPass, RenderPassCreateInfo, Subpass, SubpassDescription};

pub struct RenderPass3D {
	renderpass: Arc<RenderPass>,
}

impl RenderPass3D {
	pub fn new(device: &Arc<Device>) -> Self {
		Self {
			renderpass: RenderPass::new(device.clone(), RenderPassCreateInfo {
				attachments: vec![
					AttachmentDescription {
						format: Format::R8G8B8A8_SRGB,
						initial_layout: ImageLayout::Undefined,
						final_layout: ImageLayout::PresentSrc,
						load_op: AttachmentLoadOp::Clear,
						store_op: AttachmentStoreOp::Store,
						..Default::default()
					},
					AttachmentDescription {
						format: Format::D32_SFLOAT,
						initial_layout: ImageLayout::Undefined,
						final_layout: ImageLayout::Undefined,
						load_op: AttachmentLoadOp::Clear,
						store_op: AttachmentStoreOp::Store,
						..Default::default()
					},
				],
				subpasses: vec![
					SubpassDescription {
						color_attachments: vec![
							Some(AttachmentReference {
								attachment: 0,
								layout: ImageLayout::ColorAttachmentOptimal,
								..Default::default()
							}),
						],
						depth_stencil_attachment: Some(AttachmentReference {
							attachment: 1,
							layout: ImageLayout::DepthStencilAttachmentOptimal,
							..Default::default()
						}),
						..Default::default()
					}
				],
				..Default::default()
			}).unwrap()
		}
	}

	pub fn subpass_main(&self) -> Subpass {
		self.renderpass.clone().first_subpass()
	}
}
