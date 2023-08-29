use std::sync::Arc;

use smallvec::{SmallVec, smallvec};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderingAttachmentInfo, RenderingInfo, SubpassContents};
use vulkano::format::{ClearValue, Format};
use vulkano::image::Image;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::subpass::PipelineRenderingCreateInfo;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use crate::reinit::ReinitRef;

use crate::space::renderer::lodobj::opaque::OpaquePipeline;

pub fn rendering_info() -> PipelineRenderingCreateInfo {
	PipelineRenderingCreateInfo {
		color_attachment_formats: vec![
			// FIXME this needs to be dynamic unfortunately
			Some(Format::B8G8R8A8_SRGB),
		],
		..PipelineRenderingCreateInfo::default()
	}
}

pub struct RenderTask {
	pipeline: ReinitRef<OpaquePipeline>,
	color_attachments: SmallVec<[Vec<Option<RenderingAttachmentInfo>>; 3]>,
	viewport: SmallVec<[Viewport; 2]>,
}

impl RenderTask {
	pub fn new<'a>(pipeline: ReinitRef<OpaquePipeline>, images: impl Iterator<Item=&'a Arc<Image>>) -> Self {
		let color_attachments: SmallVec<[_; 3]> = images.map(|image| {
			vec![Some(RenderingAttachmentInfo {
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some(ClearValue::Float([0.0f32; 4])),
				..RenderingAttachmentInfo::image_view(ImageView::new_default(image.clone()).unwrap())
			})]
		}).collect();
		let image_extent: [u32; 3] = color_attachments.iter().next().unwrap().first().unwrap().as_ref().unwrap().image_view.image().extent();
		assert_eq!(1, image_extent[2]);
		Self {
			pipeline,
			color_attachments,
			viewport: smallvec![ Viewport {
				offset: [0f32, 0f32],
				extent: [image_extent[0] as f32, image_extent[1] as f32],
				depth_range: 0f32..=1f32,
			}]
		}
	}

	// TODO this is good enough for now, but later down the line when submitting to multiple queues a rethink of this method is needed
	pub fn record(&self, fif_index: usize, cmd: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
		cmd
			.begin_rendering(RenderingInfo {
				color_attachments: self.color_attachments[fif_index].clone(),
				contents: SubpassContents::Inline,
				..RenderingInfo::default()
			}).unwrap()
			.set_viewport(0, self.viewport.clone()).unwrap()
			.bind_pipeline_graphics(self.pipeline.0.clone()).unwrap()
			// FIXME do I need to set the viewport?
			// .bind_vertex_buffers(0, (**self.model).clone()).unwrap()
			.draw(3, 1, 0, 0).unwrap()
			.end_rendering().unwrap();
	}
}
