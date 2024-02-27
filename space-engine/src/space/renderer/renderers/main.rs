use std::sync::Arc;

use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::GpuFuture;

use space_engine_common::space::renderer::frame_data::FrameData;

use crate::space::renderer::lod_obj::opaque_render_task::OpaqueRenderTask;
use crate::space::renderer::model::texture_manager::TextureManager;
use crate::space::renderer::render_graph::context::{FrameContext, RenderContext, RenderContextNewFrame};
use crate::space::renderer::renderers::main::ImageNotSupportedError::{ExtendMismatch, FormatMismatch, ImageNot2D};
use crate::space::Init;

pub struct RenderPipelineMain {
	pub init: Arc<Init>,
	pub output_format: Format,
	pub depth_format: Format,
	pub opaque_task: OpaqueRenderTask,
}

impl RenderPipelineMain {
	pub fn new(init: &Arc<Init>, texture_manager: &Arc<TextureManager>, output_format: Format) -> Arc<Self> {
		// always available
		let depth_format = Format::D32_SFLOAT;

		let opaque_task = OpaqueRenderTask::new(&init, &texture_manager, output_format, depth_format);
		Arc::new(Self {
			init: init.clone(),
			output_format,
			depth_format,
			opaque_task,
		})
	}

	pub fn new_renderer(self: &Arc<Self>, extend: [u32; 3], frames_in_flight: u32) -> RendererMain {
		RendererMain::new(self.clone(), extend, frames_in_flight)
	}
}

pub struct RendererMain {
	pub pipeline: Arc<RenderPipelineMain>,
	render_context_new_frame: RenderContextNewFrame,
	resources: RendererMainResources,
}

struct RendererMainResources {
	depth_image: Arc<ImageView>,
	extent: [u32; 3],
}

impl RendererMain {
	fn new(pipeline: Arc<RenderPipelineMain>, extent: [u32; 3], frames_in_flight: u32) -> Self {
		let init = &pipeline.init;
		let (_, render_context_new_frame) = RenderContext::new(init.clone(), frames_in_flight);

		let depth_image = Image::new(
			init.memory_allocator.clone(),
			ImageCreateInfo {
				format: pipeline.depth_format,
				extent,
				usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
				..ImageCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
				allocate_preference: MemoryAllocatePreference::AlwaysAllocate,
				..AllocationCreateInfo::default()
			},
		)
		.unwrap();
		let depth_image = ImageView::new_default(depth_image).unwrap();

		let resources = RendererMainResources { extent, depth_image };
		Self {
			pipeline,
			render_context_new_frame,
			resources,
		}
	}

	pub fn new_frame<F>(&mut self, frame_data: FrameData, output_image: Arc<ImageView>, f: F)
	where
		F: FnOnce(&FrameContext, RendererMainFrame) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
	{
		self.image_supported(&output_image).unwrap();
		let extent = output_image.image().extent();
		let viewport = Viewport {
			offset: [0f32; 2],
			extent: [extent[0] as f32, extent[1] as f32],
			depth_range: 0f32..=1f32,
		};
		self.render_context_new_frame
			.new_frame(viewport, frame_data, |frame_context| {
				f(
					&frame_context,
					RendererMainFrame {
						pipeline: &self.pipeline,
						frame_context: &frame_context,
						resources: &self.resources,
						output_image,
					},
				)
			});
	}
}

pub struct RendererMainFrame<'a> {
	pipeline: &'a RenderPipelineMain,
	frame_context: &'a FrameContext<'a>,
	resources: &'a RendererMainResources,
	output_image: Arc<ImageView>,
}

impl<'a> RendererMainFrame<'a> {
	pub fn record(self, future_await: impl GpuFuture) -> impl GpuFuture {
		let r = self.resources;
		let p = &*self.pipeline;
		let c = self.frame_context;

		let future = future_await;
		let future = p.opaque_task.record(c, &self.output_image, &r.depth_image, future);
		future
	}
}

#[derive(Debug)]
pub enum ImageNotSupportedError {
	FormatMismatch { renderer: Format, image: Format },
	ImageNot2D { extent: [u32; 3] },
	ExtendMismatch { renderer: [u32; 3], image: [u32; 3] },
}

impl RendererMain {
	pub fn image_supported(&self, output_image: &Arc<ImageView>) -> Result<(), ImageNotSupportedError> {
		let extent = output_image.image().extent();
		if output_image.format() != self.pipeline.output_format {
			Err(FormatMismatch {
				renderer: self.pipeline.output_format,
				image: output_image.format(),
			})
		} else if extent[2] != 1 {
			Err(ImageNot2D { extent })
		} else if self.resources.extent != extent {
			Err(ExtendMismatch {
				renderer: self.resources.extent,
				image: extent,
			})
		} else {
			Ok(())
		}
	}
}
