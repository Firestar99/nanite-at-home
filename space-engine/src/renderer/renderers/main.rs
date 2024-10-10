use crate::renderer::lighting::lighting_render_task::LightingRenderTask;
use crate::renderer::meshlet::meshlet_render_task::MeshletRenderTask;
use crate::renderer::render_graph::context::{FrameContext, RenderContext, RenderContextNewFrame};
use crate::renderer::renderers::main::ImageNotSupportedError::{ExtendMismatch, FormatMismatch, ImageNot2D};
use crate::renderer::Init;
use space_engine_shader::renderer::frame_data::FrameData;
use std::sync::Arc;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateFlags, ImageCreateInfo, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::GpuFuture;
use vulkano_bindless::frame_manager::PrevFrameFuture;

pub struct RenderPipelineMain {
	pub init: Arc<Init>,

	pub output_format: Format,
	pub g_albedo_format: Format,
	pub g_normal_format: Format,
	pub g_rm_format: Format,
	pub depth_format: Format,

	pub meshlet_task: MeshletRenderTask,
	pub lighting_task: LightingRenderTask,
}

impl RenderPipelineMain {
	pub fn new(init: &Arc<Init>, output_format: Format) -> Arc<Self> {
		// all formats are always available
		let depth_format = Format::D32_SFLOAT;
		let g_albedo_format = Format::R8G8B8A8_SRGB;
		let g_normal_format = Format::R16G16B16A16_SFLOAT;
		let g_rm_format = Format::R16G16_SFLOAT;

		let meshlet_task = MeshletRenderTask::new(init, g_albedo_format, g_normal_format, g_rm_format, depth_format);
		let lighting_task = LightingRenderTask::new(init);
		Arc::new(Self {
			init: init.clone(),
			output_format,
			g_albedo_format,
			g_normal_format,
			g_rm_format,
			depth_format,
			meshlet_task,
			lighting_task,
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
	g_albedo_image: Arc<ImageView>,
	g_normal_image: Arc<ImageView>,
	g_rm_image: Arc<ImageView>,
	depth_image: Arc<ImageView>,
	extent: [u32; 3],
}

impl RendererMain {
	fn new(pipeline: Arc<RenderPipelineMain>, extent: [u32; 3], frames_in_flight: u32) -> Self {
		let init = &pipeline.init;
		let (_, render_context_new_frame) = RenderContext::new(init.clone(), frames_in_flight);

		let create_image = |format: Format, usage: ImageUsage, flags: ImageCreateFlags| {
			Image::new(
				init.memory_allocator.clone(),
				ImageCreateInfo {
					flags,
					format,
					extent,
					usage,
					..ImageCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
					allocate_preference: MemoryAllocatePreference::AlwaysAllocate,
					..AllocationCreateInfo::default()
				},
			)
			.unwrap()
		};

		let g_albedo_image = ImageView::new_default(create_image(
			pipeline.g_albedo_format,
			ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
			ImageCreateFlags::empty(),
		))
		.unwrap();
		let g_normal_image = ImageView::new_default(create_image(
			pipeline.g_normal_format,
			ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
			ImageCreateFlags::empty(),
		))
		.unwrap();
		let g_rm_image = ImageView::new_default(create_image(
			pipeline.g_rm_format,
			ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
			ImageCreateFlags::empty(),
		))
		.unwrap();
		let depth_image = ImageView::new_default(create_image(
			pipeline.depth_format,
			ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
			ImageCreateFlags::empty(),
		))
		.unwrap();

		Self {
			pipeline,
			render_context_new_frame,
			resources: RendererMainResources {
				extent,
				depth_image,
				g_albedo_image,
				g_normal_image,
				g_rm_image,
			},
		}
	}

	pub fn new_frame<F>(&mut self, frame_data: FrameData, output_image: Arc<ImageView>, f: F)
	where
		F: FnOnce(&FrameContext, PrevFrameFuture, RendererMainFrame) -> Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
	{
		self.image_supported(&output_image).unwrap();
		let extent = output_image.image().extent();
		let viewport = Viewport {
			offset: [0f32; 2],
			extent: [extent[0] as f32, extent[1] as f32],
			depth_range: 0f32..=1f32,
		};
		self.render_context_new_frame
			.new_frame(viewport, frame_data, |frame_context, prev_frame_future| {
				f(
					frame_context,
					prev_frame_future,
					RendererMainFrame {
						pipeline: &self.pipeline,
						frame_context,
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
	#[profiling::function]
	pub fn record(self, future: impl GpuFuture) -> impl GpuFuture {
		let r = self.resources;
		let p = self.pipeline;
		let c = self.frame_context;

		let future = p.meshlet_task.record(
			c,
			&r.g_albedo_image,
			&r.g_normal_image,
			&r.g_rm_image,
			&r.depth_image,
			future,
		);

		p.lighting_task.record(
			c,
			&r.g_albedo_image,
			&r.g_normal_image,
			&r.g_rm_image,
			&r.depth_image,
			&self.output_image,
			future,
		)
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
