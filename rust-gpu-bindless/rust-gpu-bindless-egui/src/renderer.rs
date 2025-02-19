use crate::platform::EguiBindlessPlatform;
use ash::vk::{
	BlendFactor, BlendOp, ColorComponentFlags, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineRasterizationStateCreateInfo,
	PrimitiveTopology,
};
use egui::epaint::Primitive;
use egui::{ClippedPrimitive, Context, TexturesDelta};
use glam::{IVec2, UVec2};
use rust_gpu_bindless::generic::descriptor::{Bindless, Format, Image2d};
use rust_gpu_bindless::generic::pipeline::{
	BindlessGraphicsPipeline, GraphicsPipelineCreateInfo, Recording, RecordingError, RenderPassFormat,
	RenderingAttachment,
};
use rust_gpu_bindless::generic::pipeline::{ColorAttachment, DepthStencilAttachment, LoadOp, MutImageAccess, StoreOp};
use rust_gpu_bindless_egui_shaders::Param;
use rust_gpu_bindless_shaders::utils::rect::IRect2;
use rust_gpu_bindless_shaders::utils::viewport::Viewport;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use thiserror::Error;

pub struct EguiRenderer<P: EguiBindlessPlatform> {
	bindless: Arc<Bindless<P>>,
	output_format: Format,
	depth_format: Option<Format>,
	graphics_pipeline: BindlessGraphicsPipeline<P, Param<'static>>,
	// white_texture: RCDesc<P, Image<Image2d>>,
}

impl<P: EguiBindlessPlatform> EguiRenderer<P> {
	pub fn new(bindless: Arc<Bindless<P>>, output_format: Format, depth_format: Option<Format>) -> Arc<Self> {
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

pub struct EguiRenderContext<P: EguiBindlessPlatform> {
	renderer: Arc<EguiRenderer<P>>,
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

impl<P: EguiBindlessPlatform> EguiRenderContext<P> {
	pub fn new(renderer: Arc<EguiRenderer<P>>, ctx: Context) -> Self {
		Self { renderer, ctx }
	}

	pub fn update_texture(&mut self, texture_delta: TexturesDelta) {}

	pub fn draw<'a>(
		&mut self,
		cmd: &mut Recording<'a, P>,
		image: &mut MutImageAccess<'a, P, Image2d, ColorAttachment>,
		depth: Option<&mut MutImageAccess<'a, P, Image2d, DepthStencilAttachment>>,
		options: RenderingOptions,
		geometry: Vec<ClippedPrimitive>,
	) -> Result<(), EguiRenderingError<P>> {
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
			|rp| {
				let mut prev_clip_rect = None;
				for primitive in geometry {
					{
						let rect = primitive.clip_rect;
						let rect = IRect2 {
							origin: IVec2::new(rect.min.x.floor() as i32, rect.min.y.floor() as i32),
							extent: UVec2::new(rect.width().ceil() as u32, rect.height().ceil() as u32),
						};
						if prev_clip_rect.map_or(true, |prev| prev != rect) {
							prev_clip_rect = Some(rect);
							rp.set_viewport(Viewport {
								x: rect.origin.x as f32,
								y: rect.origin.y as f32,
								width: rect.extent.x as f32,
								height: rect.extent.y as f32,
								min_depth: 0.0,
								max_depth: 1.0,
							});
							rp.set_scissor(rect);
						}
					}

					match primitive.primitive {
						Primitive::Mesh(mesh) => {}
						Primitive::Callback(_) => {
							panic!("callback unsupported")
						}
					}
				}

				Ok(())
			},
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

impl<P: EguiBindlessPlatform> Deref for EguiRenderContext<P> {
	type Target = Context;

	fn deref(&self) -> &Self::Target {
		&self.ctx
	}
}

impl<P: EguiBindlessPlatform> DerefMut for EguiRenderContext<P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.ctx
	}
}

pub struct RenderOutput<'a, P: EguiBindlessPlatform> {
	render_ctx: &'a mut EguiRenderContext<P>,
	primitives: Vec<ClippedPrimitive>,
}

impl<'a, P: EguiBindlessPlatform> RenderOutput<'a, P> {
	pub fn draw(&self) {}
}
