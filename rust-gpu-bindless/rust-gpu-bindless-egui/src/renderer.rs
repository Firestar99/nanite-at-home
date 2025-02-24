use crate::convert::Egui2Bindless;
use crate::platform::EguiBindlessPlatform;
use ash::vk::{
	BlendFactor, BlendOp, ColorComponentFlags, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineRasterizationStateCreateInfo,
	PrimitiveTopology,
};
use egui::epaint::Primitive;
use egui::{Context, FullOutput, ImageData, TextureId, TextureOptions, TexturesDelta};
use glam::{IVec2, UVec2};
use rust_gpu_bindless::generic::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, BindlessSamplerCreateInfo, BufferAllocationError, Extent, Filter, Format, Image2d,
	MutBoxDescExt, MutDesc, MutDescBufferExt, MutDescExt, MutImage, RCDesc, RCDescExt, Sampler, SamplerAllocationError,
};
use rust_gpu_bindless::generic::pipeline::{
	BindlessGraphicsPipeline, ColorAttachment, DepthStencilAttachment, GraphicsPipelineCreateInfo, LoadOp,
	MutBufferAccessExt, MutImageAccess, MutImageAccessExt, Recording, RecordingError, RenderPassFormat,
	RenderingAttachment, StoreOp, TransferRead, TransferWrite,
};
use rust_gpu_bindless_egui_shaders::{ImageVertex, Param, ParamFlags, Vertex};
use rust_gpu_bindless_shaders::descriptor::{Buffer, UnsafeDesc};
use rust_gpu_bindless_shaders::spirv_std::indirect_command::DrawIndexedIndirectCommand;
use rust_gpu_bindless_shaders::utils::rect::IRect2;
use rust_gpu_bindless_shaders::utils::viewport::Viewport;
use rustc_hash::FxHashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use thiserror::Error;

/// The global egui renderer, contains the graphics pipelines required for rendering. Create it once per required
/// render target format.
pub struct EguiRenderer<P: EguiBindlessPlatform> {
	bindless: Arc<Bindless<P>>,
	output_format: Format,
	depth_format: Option<Format>,
	graphics_pipeline: BindlessGraphicsPipeline<P, Param<'static>>,
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

/// The RenderContext is the renderer for a particular egui [`Context`], create once per egui [`Context`]. Mixing
/// different contexts or their [`FullOutput`]s may lead to panics and state / image corruption.
pub struct EguiRenderContext<P: EguiBindlessPlatform> {
	renderer: Arc<EguiRenderer<P>>,
	ctx: Context,
	textures: FxHashMap<TextureId, (MutDesc<P, MutImage<Image2d>>, RCDesc<P, Sampler>)>,
	textures_free_queued: Vec<TextureId>,
	/// deduplicate samplers, they will never be freed if unused
	samplers: FxHashMap<TextureOptions, RCDesc<P, Sampler>>,
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
		Self {
			renderer,
			ctx,
			textures: FxHashMap::default(),
			textures_free_queued: Vec::new(),
			samplers: FxHashMap::default(),
		}
	}

	pub fn update(&mut self, output: FullOutput) -> Result<RenderOutput<P>, EguiRenderingError<P>> {
		self.free_textures(&output.textures_delta)?;
		self.update_textures(&output.textures_delta)?;
		Ok(self.tessellate_upload(output)?)
	}

	fn free_textures(&mut self, texture_delta: &TexturesDelta) -> Result<(), EguiRenderingError<P>> {
		for texture_id in &self.textures_free_queued {
			self.textures.remove(&texture_id);
		}
		self.textures_free_queued.clear();
		self.textures_free_queued.extend_from_slice(&texture_delta.free);
		Ok(())
	}

	fn update_textures(&mut self, texture_delta: &TexturesDelta) -> Result<(), EguiRenderingError<P>> {
		if texture_delta.set.is_empty() {
			return Ok(());
		}

		// FIXME copy MUST wait for last draw to finish executing
		self.renderer.bindless.execute(|cmd| {
			for (id, delta) in &texture_delta.set {
				let id = *id;
				match &delta.pos {
					None => {
						let extent = Extent::from([delta.image.width() as u32, delta.image.height() as u32]);
						let (format, bytes): (Format, &[u8]) = match &delta.image {
							// format must be UNORM even though color data is SRGB, so that sample ops return colors in srgb colorspace
							ImageData::Color(color) => (Format::R8G8B8A8_UNORM, bytemuck::cast_slice(&color.pixels)),
							ImageData::Font(font) => (Format::R32_SFLOAT, bytemuck::cast_slice(&font.pixels)),
						};

						let staging = self
							.renderer
							.bindless
							.buffer()
							.alloc_from_iter(
								&BindlessBufferCreateInfo {
									usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::TRANSFER_SRC,
									allocation_scheme: Default::default(),
									name: &format!("egui texture {:?} staging buffer", id),
								},
								bytes.iter().copied(),
							)
							.unwrap();

						let image = self
							.renderer
							.bindless
							.image()
							.alloc(&BindlessImageCreateInfo {
								format,
								extent,
								mip_levels: 0,
								array_layers: 0,
								samples: Default::default(),
								usage: BindlessImageUsage::TRANSFER_DST | BindlessImageUsage::SAMPLED,
								allocation_scheme: Default::default(),
								name: &format!("egui texture {:?}", id),
								_phantom: Default::default(),
							})
							.unwrap();

						let image = image.access_dont_care::<TransferWrite>(cmd)?;
						let staging = staging.access::<TransferRead>(cmd)?;
						cmd.copy_buffer_to_image(&staging, &image)?;
						let image = image.into_desc();

						let sampler = self
							.samplers
							.entry(delta.options)
							.or_insert_with(|| {
								self.renderer
									.bindless
									.sampler()
									.alloc(&BindlessSamplerCreateInfo {
										min_filter: delta.options.minification.to_bindless(),
										mag_filter: delta.options.magnification.to_bindless(),
										mipmap_mode: delta
											.options
											.mipmap_mode
											.map_or(Filter::Nearest, |f| f.to_bindless()),
										address_mode_v: delta.options.wrap_mode.to_bindless(),
										address_mode_u: delta.options.wrap_mode.to_bindless(),
										address_mode_w: delta.options.wrap_mode.to_bindless(),
										..BindlessSamplerCreateInfo::default()
									})
									.unwrap()
							})
							.clone();

						self.textures.insert(id, (image, sampler));
					}
					Some(_pos) => {
						unimplemented!()
					}
				}
			}
			Ok(())
		})?;
		Ok(())
	}

	fn tessellate_upload(&mut self, output: FullOutput) -> Result<RenderOutput<P>, EguiRenderingError<P>> {
		let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
		let (vertex_cnt, index_cnt) = primitives
			.iter()
			.map(|p| match &p.primitive {
				Primitive::Mesh(mesh) => (mesh.vertices.len(), mesh.indices.len()),
				Primitive::Callback(_) => (0, 0),
			})
			.fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));

		let vertices = self.renderer.bindless.buffer().alloc_slice(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				name: "egui vertex buffer",
			},
			vertex_cnt,
		)?;

		let indices = self.renderer.bindless.buffer().alloc_slice(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::INDEX_BUFFER,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				name: "egui index buffer",
			},
			index_cnt,
		)?;

		let mut draw_cmds: Vec<DrawCmd> = Vec::new();
		{
			let mut vertices = vertices.mapped_immediate().unwrap();
			let mut indices = indices.mapped_immediate().unwrap();
			let mut vertices_idx = 0;
			let mut indices_idx = 0;
			for p in primitives {
				match p.primitive {
					Primitive::Mesh(mesh) => {
						let (image, sampler) = self
							.textures
							.get(&mesh.texture_id)
							.ok_or(EguiRenderingError::MissingTexture(mesh.texture_id))?;
						let image = unsafe { UnsafeDesc::new(image.id()) };
						let sampler = sampler.to_strong();

						let vertices_start = vertices_idx;
						let indices_start = indices_idx;
						for vertex in mesh.vertices {
							vertices.write_offset(
								vertices_idx,
								ImageVertex {
									vertex: Vertex::from(vertex),
									image,
									sampler,
								},
							);
							vertices_idx += 1;
						}
						for index in mesh.indices {
							indices.write_offset(indices_idx, index + vertices_start as u32);
							indices_idx += 1;
						}

						let rect = p.clip_rect;
						let rect = IRect2 {
							origin: IVec2::new(rect.min.x.floor() as i32, rect.min.y.floor() as i32),
							extent: UVec2::new(rect.width().ceil() as u32, rect.height().ceil() as u32),
						};

						if let Some(last) = draw_cmds.last_mut().filter(|last| last.clip_rect == rect) {
							// reuse last draw call
							last.indices_count += (indices_idx - indices_start) as u32;
						} else {
							// new draw call
							draw_cmds.push(DrawCmd {
								clip_rect: rect,
								indices_offset: indices_start as u32,
								indices_count: (indices_idx - indices_start) as u32,
							})
						}
					}
					Primitive::Callback(_) => (),
				}
			}

			assert_eq!(vertices_idx, vertex_cnt);
			assert_eq!(indices_idx, index_cnt);
		}

		// Safety: these buffers have never been used in any cmd
		let vertices = unsafe { vertices.into_shared_unchecked() };
		let indices = unsafe { indices.into_shared_unchecked() };

		Ok(RenderOutput {
			render_ctx: self,
			vertices,
			indices,
			draw_cmds,
		})
	}
}

pub struct RenderOutput<'a, P: EguiBindlessPlatform> {
	render_ctx: &'a mut EguiRenderContext<P>,
	vertices: RCDesc<P, Buffer<[ImageVertex]>>,
	indices: RCDesc<P, Buffer<[u32]>>,
	draw_cmds: Vec<DrawCmd>,
}

struct DrawCmd {
	clip_rect: IRect2,
	indices_offset: u32,
	indices_count: u32,
}

impl<'b, P: EguiBindlessPlatform> RenderOutput<'b, P> {
	pub fn draw<'a>(
		&mut self,
		cmd: &mut Recording<'a, P>,
		image: &mut MutImageAccess<'a, P, Image2d, ColorAttachment>,
		depth: Option<&mut MutImageAccess<'a, P, Image2d, DepthStencilAttachment>>,
		options: RenderingOptions,
	) -> Result<(), EguiRenderingError<P>> {
		let depth = match (self.render_ctx.renderer.depth_format, depth) {
			(Some(format), Some(depth)) => Some((format, depth)),
			(None, None) => None,
			(None, Some(_)) => return Err(EguiRenderingError::UnexpectedDepthRT),
			(Some(format), None) => return Err(EguiRenderingError::ExpectedDepthRT(format)),
		};

		let param = Param {
			vertices: self.vertices.to_transient(cmd),
			flags: ParamFlags::SRGB_FRAMEBUFFER,
		};

		cmd.begin_rendering(
			self.render_ctx.renderer.render_pass_format(),
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
				for draw in &self.draw_cmds {
					let rect = draw.clip_rect;
					rp.set_viewport(Viewport {
						x: rect.origin.x as f32,
						y: rect.origin.y as f32,
						width: rect.extent.x as f32,
						height: rect.extent.y as f32,
						min_depth: 0.0,
						max_depth: 1.0,
					});
					rp.set_scissor(rect);
					rp.draw_indexed(
						&self.render_ctx.renderer.graphics_pipeline,
						&self.indices,
						DrawIndexedIndirectCommand {
							index_count: draw.indices_count,
							instance_count: 1,
							first_index: draw.indices_offset,
							vertex_offset: 0,
							first_instance: 0,
						},
						param,
					)?;
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
	#[error("Expected depth render target in format {0:?}, but got None")]
	ExpectedDepthRT(Format),
	#[error("Expected no render target, but got a image")]
	UnexpectedDepthRT,
	#[error("TextureId {0:?} does not exist in the rendering backend. Has this `EguiRenderContext` been used with multiple egui `Context`s?")]
	MissingTexture(TextureId),
	#[error("BufferAllocationError: {0}")]
	BufferAllocationError(#[from] BufferAllocationError<P>),
	#[error("ImageAllocationError: {0}")]
	ImageAllocationError(#[from] SamplerAllocationError<P>),
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
