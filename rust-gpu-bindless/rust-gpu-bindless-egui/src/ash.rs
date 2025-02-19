use crate::EguiBindlessPlatform;
use ash::vk::{
	BlendFactor, BlendOp, ColorComponentFlags, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineRasterizationStateCreateInfo,
	PrimitiveTopology,
};
use egui::{ClippedPrimitive, Context, TexturesDelta};
use rust_gpu_bindless::generic::descriptor::{Bindless, Format, Image2d};
use rust_gpu_bindless::generic::pipeline::{
	BindlessGraphicsPipeline, GraphicsPipelineCreateInfo, Recording, RecordingError, RenderPassFormat,
	RenderingAttachment,
};
use rust_gpu_bindless::generic::pipeline::{ColorAttachment, DepthStencilAttachment, LoadOp, MutImageAccess, StoreOp};
use rust_gpu_bindless::generic::platform::ash::Ash;
use rust_gpu_bindless_egui_shaders::Param;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use thiserror::Error;

unsafe impl EguiBindlessPlatform for Ash {
	unsafe fn max_image_dimensions_2d(&self) -> u32 {
		unsafe {
			self.instance
				.get_physical_device_properties(self.physical_device)
				.limits
				.max_image_dimension2_d
		}
	}
}

pub struct EguiAshRenderer {
	bindless: Arc<Bindless<Ash>>,
	output_format: Format,
	depth_format: Option<Format>,
	graphics_pipeline: BindlessGraphicsPipeline<Ash, Param<'static>>,
}

impl EguiAshRenderer {
	pub fn new(bindless: Arc<Bindless<Ash>>, output_format: Format, depth_format: Option<Format>) -> Arc<Self> {
		let format = RenderPassFormat::new(&[output_format], depth_format);
		let graphics_pipeline = bindless
			.create_graphics_pipeline(
				&format,
				&GraphicsPipelineCreateInfo {
					input_assembly_state: PipelineInputAssemblyStateCreateInfo::default()
						.topology(PrimitiveTopology::TRIANGLE_LIST),
					rasterization_state: PipelineRasterizationStateCreateInfo::default().line_width(1.0),
					depth_stencil_state: PipelineDepthStencilStateCreateInfo::default()
						.depth_test_enable(false)
						.depth_write_enable(true),
					color_blend_state: PipelineColorBlendStateCreateInfo::default().attachments(&[
						PipelineColorBlendAttachmentState::default()
							.blend_enable(true)
							.src_color_blend_factor(BlendFactor::ONE)
							.color_blend_op(BlendOp::ADD)
							.dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
							.src_alpha_blend_factor(BlendFactor::ONE)
							.alpha_blend_op(BlendOp::ADD)
							.dst_alpha_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
							.color_write_mask(ColorComponentFlags::RGBA),
					]),
				},
				crate::shaders::egui_vertex::new(),
				crate::shaders::egui_fragment::new(),
			)
			.unwrap();
		Arc::new(Self {
			bindless,
			output_format,
			depth_format,
			graphics_pipeline,
		})
	}

	pub fn render_pass_format(&self) -> RenderPassFormat {
		RenderPassFormat::new(&[self.output_format], self.depth_format)
	}
}

pub struct EguiAshRenderContext {
	renderer: Arc<EguiAshRenderer>,
	ctx: Context,
}

pub struct RenderingOptions {
	image_rt_load_op: LoadOp,
	depth_rt_load_op: LoadOp,
}

impl Default for RenderingOptions {
	fn default() -> Self {
		Self {
			image_rt_load_op: LoadOp::Load,
			depth_rt_load_op: LoadOp::Load,
		}
	}
}

impl EguiAshRenderContext {
	pub fn new(renderer: Arc<EguiAshRenderer>, ctx: Context) -> Self {
		Self { renderer, ctx }
	}

	pub fn update_texture(&self, texture_delta: TexturesDelta) {}

	pub fn draw<'a>(
		&self,
		cmd: &mut Recording<'a, Ash>,
		image: &mut MutImageAccess<'a, Ash, Image2d, ColorAttachment>,
		depth: Option<&mut MutImageAccess<'a, Ash, Image2d, DepthStencilAttachment>>,
		options: RenderingOptions,
	) -> Result<(), EguiRenderingError<Ash>> {
		let depth = match (self.renderer.depth_format, depth) {
			(Some(format), Some(depth)) => Some((format, depth)),
			(None, None) => None,
			(None, Some(_)) => return Err(EguiRenderingError::UnexpectedDepthTexture),
			(Some(format), None) => return Err(EguiRenderingError::ExpectedDepthTexture(format)),
		};

		cmd.begin_rendering(
			self.renderer.render_pass_format(),
			&[RenderingAttachment {
				image,
				load_op: options.image_rt_load_op,
				store_op: StoreOp::Store,
			}],
			depth.map(|(_, depth)| RenderingAttachment {
				image: depth,
				load_op: options.depth_rt_load_op,
				store_op: StoreOp::Store,
			}),
			|rp| Ok(()),
		)?;

		Ok(())
	}
}

#[derive(Error)]
pub enum EguiRenderingError<P: EguiBindlessPlatform> {
	#[error("Recording Error: {0}")]
	RecordingError(#[from] RecordingError<P>),
	#[error("Expected depth texture in format {0:?}, but got None")]
	ExpectedDepthTexture(Format),
	#[error("Expected no depth texture, but got a texture")]
	UnexpectedDepthTexture,
}

impl<P: EguiBindlessPlatform> core::fmt::Debug for EguiRenderingError<P> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(self, f)
	}
}

impl Deref for EguiAshRenderContext {
	type Target = Context;

	fn deref(&self) -> &Self::Target {
		&self.ctx
	}
}

impl DerefMut for EguiAshRenderContext {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.ctx
	}
}

pub struct RenderOutput<'a> {
	render_ctx: &'a mut EguiAshRenderContext,
	primitives: Vec<ClippedPrimitive>,
}

impl<'a> RenderOutput<'a> {
	pub fn draw(&self) {}
}
