use crate::convert::Egui2Bindless;
use crate::platform::EguiBindlessPlatform;
use ash::vk::{
	BlendFactor, BlendOp, ColorComponentFlags, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
	PipelineDepthStencilStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineRasterizationStateCreateInfo,
	PrimitiveTopology,
};
use egui::epaint::Primitive;
use egui::{
	Context, FullOutput, ImageData, PlatformOutput, RawInput, Rect, TextureId, TextureOptions, TexturesDelta, epaint,
};
use glam::{IVec2, UVec2};
use parking_lot::Mutex;
use rust_gpu_bindless_core::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, BindlessImageCreateInfo,
	BindlessImageUsage, BindlessSamplerCreateInfo, BufferAllocationError, Extent, Filter, Format, Image2d,
	ImageDescExt, MutBoxDescExt, MutDesc, MutDescBufferExt, MutDescExt, MutImage, RCDesc, RCDescExt, Sampler,
	SamplerAllocationError,
};
use rust_gpu_bindless_core::pipeline::{
	BindlessGraphicsPipeline, ColorAttachment, DepthStencilAttachment, GraphicsPipelineCreateInfo, HasResourceContext,
	ImageAccessType, LoadOp, MutBufferAccessExt, MutImageAccess, MutImageAccessExt, Recording, RecordingError,
	RenderPassFormat, RenderingAttachment, StoreOp, TransferRead, TransferWrite,
};
use rust_gpu_bindless_core::platform::RecordingResourceContext;
use rust_gpu_bindless_egui_shaders::{Param, ParamFlags, Vertex};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Image, UnsafeDesc};
use rust_gpu_bindless_shaders::spirv_std::indirect_command::DrawIndexedIndirectCommand;
use rust_gpu_bindless_shaders::utils::rect::IRect2;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;

/// The global `EguiRenderer` should be created exactly once and can create new [`EguiRenderPipeline`] and [`EguiRenderContext`].
pub struct EguiRenderer<P: EguiBindlessPlatform>(Arc<EguiRendererInner<P>>);

impl<P: EguiBindlessPlatform> Clone for EguiRenderer<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P: EguiBindlessPlatform> Deref for EguiRenderer<P> {
	type Target = EguiRendererInner<P>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct EguiRendererInner<P: EguiBindlessPlatform> {
	bindless: Bindless<P>,
	/// deduplicate samplers, they will never be freed if unused
	samplers: Mutex<FxHashMap<TextureOptions, RCDesc<P, Sampler>>>,
}

impl<P: EguiBindlessPlatform> EguiRenderer<P> {
	pub fn new(bindless: Bindless<P>) -> Self {
		EguiRenderer(Arc::new(EguiRendererInner {
			bindless,
			samplers: Mutex::new(FxHashMap::default()),
		}))
	}

	pub fn bindless(&self) -> &Bindless<P> {
		&self.bindless
	}
}

/// `EguiRenderPipeline` represents the graphics pipelines used to draw the ui onto some image. Create a new
/// `EguiRenderPipeline` for each combination of color and depth rendertarget that should be rendered onto. May be
/// used by multiple [`EguiRenderContext`], even simultaneously.
pub struct EguiRenderPipeline<P: EguiBindlessPlatform>(Arc<EguiRenderPipelineInner<P>>);

impl<P: EguiBindlessPlatform> Clone for EguiRenderPipeline<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P: EguiBindlessPlatform> Deref for EguiRenderPipeline<P> {
	type Target = EguiRenderPipelineInner<P>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct EguiRenderPipelineInner<P: EguiBindlessPlatform> {
	renderer: EguiRenderer<P>,
	color_format: Option<Format>,
	depth_format: Option<Format>,
	graphics_pipeline: BindlessGraphicsPipeline<P, Param<'static>>,
}

impl<P: EguiBindlessPlatform> EguiRenderPipeline<P> {
	pub fn new(renderer: EguiRenderer<P>, color_format: Option<Format>, depth_format: Option<Format>) -> Self {
		assert!(color_format.is_some() || depth_format.is_some());
		let bindless = &renderer.bindless;
		let format = RenderPassFormat::new(color_format.as_slice(), depth_format);
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
		EguiRenderPipeline(Arc::new(EguiRenderPipelineInner {
			renderer,
			color_format,
			depth_format,
			graphics_pipeline,
		}))
	}

	pub fn renderer(&self) -> &EguiRenderer<P> {
		&self.renderer
	}

	pub fn render_pass_format(&self) -> RenderPassFormat {
		RenderPassFormat::new(self.color_format.as_slice(), self.depth_format)
	}
}

/// The `EguiRenderContext` is the renderer for one particular egui [`Context`]. Use [`Self::run`] to run the
/// [`Context`] or manually [`Self::update`] it using the [`FullOutput`] returned by egui. You must not mix the
/// [`FullOutput`] of different contexts, as that may lead to panics and state / image corruption.
pub struct EguiRenderContext<P: EguiBindlessPlatform> {
	renderer: EguiRenderer<P>,
	ctx: Context,
	textures: FxHashMap<TextureId, EguiTexture<P>>,
	textures_free_queued: Vec<TextureId>,
	upload_wait: Mutex<SmallVec<[P::PendingExecution; 2]>>,
}

impl<P: EguiBindlessPlatform> Deref for EguiRenderContext<P> {
	type Target = Context;

	fn deref(&self) -> &Self::Target {
		&self.ctx
	}
}

pub enum EguiTextureType {
	Color,
	Font,
}

impl EguiTextureType {
	pub fn to_ash_format(&self) -> Format {
		match self {
			EguiTextureType::Color => Format::R8G8B8A8_UNORM,
			EguiTextureType::Font => Format::R32_SFLOAT,
		}
	}

	pub fn to_vertex_flags(&self) -> ParamFlags {
		match self {
			EguiTextureType::Color => ParamFlags::empty(),
			EguiTextureType::Font => ParamFlags::FONT_TEXTURE,
		}
	}
}

pub struct EguiTexture<P: EguiBindlessPlatform> {
	image: MutDesc<P, MutImage<Image2d>>,
	sampler: RCDesc<P, Sampler>,
	texture_type: EguiTextureType,
}

impl<P: EguiBindlessPlatform> EguiRenderContext<P> {
	pub fn new(renderer: EguiRenderer<P>, ctx: Context) -> Self {
		Self {
			renderer,
			ctx,
			textures: FxHashMap::default(),
			textures_free_queued: Vec::new(),
			upload_wait: Mutex::new(SmallVec::new()),
		}
	}

	pub fn renderer(&self) -> &EguiRenderer<P> {
		&self.renderer
	}

	/// Runs the ui using the supplied `run_ui` function and calls [`Self::update`] using the returned [`FullOutput`].
	/// Returns an [`EguiRenderOutput`] that may be used to draw the ui geometry and the [`PlatformOutput`] of the
	/// [`FullOutput`].
	pub fn run(
		&mut self,
		new_input: RawInput,
		run_ui: impl FnMut(&Context),
	) -> Result<(EguiRenderOutput<'_, P>, PlatformOutput), EguiRenderingError<P>> {
		profiling::function_scope!();
		let full_output = {
			profiling::scope!("egui::Context::run");
			self.ctx.run(new_input, run_ui)
		};
		self.update(full_output)
	}

	/// Updates the textures and geometry of the renderer according to the [`FullOutput`].
	/// Returns an [`EguiRenderOutput`] that may be used to draw the ui geometry and the [`PlatformOutput`] of the
	/// [`FullOutput`].
	pub fn update(
		&mut self,
		output: FullOutput,
	) -> Result<(EguiRenderOutput<'_, P>, PlatformOutput), EguiRenderingError<P>> {
		profiling::function_scope!();
		self.free_textures(&output.textures_delta)?;
		self.update_textures(&output.textures_delta)?;
		let render_output = self.tessellate_upload(output.shapes, output.pixels_per_point)?;
		Ok((render_output, output.platform_output))
	}

	fn free_textures(&mut self, texture_delta: &TexturesDelta) -> Result<(), EguiRenderingError<P>> {
		profiling::function_scope!();
		for texture_id in &self.textures_free_queued {
			self.textures.remove(texture_id);
		}
		self.textures_free_queued.clear();
		self.textures_free_queued.extend_from_slice(&texture_delta.free);
		Ok(())
	}

	fn update_textures(&mut self, texture_delta: &TexturesDelta) -> Result<(), EguiRenderingError<P>> {
		profiling::function_scope!();
		if texture_delta.set.is_empty() {
			return Ok(());
		}

		let bindless = &self.renderer.bindless;
		bindless.execute(|cmd| {
			// lock must always immediately succeed, as &mut prevents rendering() from accessing it
			for exec in self.upload_wait.try_lock().unwrap().drain(..) {
				cmd.resource_context().add_dependency(exec);
			}

			for (id, delta) in &texture_delta.set {
				let id = *id;
				match &delta.pos {
					None => {
						profiling::scope!("alloc Texture", &format!("{:?}", id));
						let extent = Extent::from([delta.image.width() as u32, delta.image.height() as u32]);
						let (texture_type, bytes): (EguiTextureType, &[u8]) = match &delta.image {
							// format must be UNORM even though color data is SRGB, so that sample ops return colors in srgb colorspace
							ImageData::Color(color) => (EguiTextureType::Color, bytemuck::cast_slice(&color.pixels)),
							ImageData::Font(font) => (EguiTextureType::Font, bytemuck::cast_slice(&font.pixels)),
						};

						let staging = bindless
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

						let image = bindless
							.image()
							.alloc(&BindlessImageCreateInfo {
								format: texture_type.to_ash_format(),
								extent,
								mip_levels: 1,
								array_layers: 1,
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

						let sampler = {
							// I would not hold a lock across the loop, we will rarely allocate a new texture
							let mut samplers = self.renderer.samplers.lock();
							let sampler = samplers.entry(delta.options).or_insert_with(|| {
								bindless
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
							});
							sampler.clone()
						};

						self.textures.insert(
							id,
							EguiTexture {
								image,
								sampler,
								texture_type,
							},
						);
					}
					Some(_pos) => {
						profiling::scope!("update Texture", &format!("{:?}", id));
						// unimplemented!()
					}
				}
			}
			Ok(())
		})?;
		Ok(())
	}

	fn tessellate_upload(
		&mut self,
		shapes: Vec<epaint::ClippedShape>,
		pixels_per_point: f32,
	) -> Result<EguiRenderOutput<'_, P>, EguiRenderingError<P>> {
		profiling::function_scope!();
		if shapes.is_empty() {
			return Ok(EguiRenderOutput::empty(self));
		}

		let primitives = {
			profiling::scope!("egui::Context::tessellate");
			self.ctx.tessellate(shapes, pixels_per_point)
		};

		let (vertex_cnt, index_cnt) = primitives
			.iter()
			.map(|p| match &p.primitive {
				Primitive::Mesh(mesh) => (mesh.vertices.len(), mesh.indices.len()),
				Primitive::Callback(_) => (0, 0),
			})
			.fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
		if vertex_cnt == 0 || index_cnt == 0 {
			return Ok(EguiRenderOutput::empty(self));
		}

		let bindless = &self.renderer.bindless;
		let vertices = bindless.buffer().alloc_slice(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::STORAGE_BUFFER,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				name: "egui vertex buffer",
			},
			vertex_cnt,
		)?;
		let indices = bindless.buffer().alloc_slice(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::MAP_WRITE | BindlessBufferUsage::INDEX_BUFFER,
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
				name: "egui index buffer",
			},
			index_cnt,
		)?;

		let mut draw_cmds: Vec<DrawCmd> = Vec::with_capacity(primitives.len());
		{
			profiling::scope!("write DrawCmd and primitives");
			let mut vertices = vertices.mapped_immediate().unwrap();
			let mut indices = indices.mapped_immediate().unwrap();
			let mut vertices_idx = 0;
			let mut indices_idx = 0;
			for p in primitives {
				match p.primitive {
					Primitive::Mesh(mesh) => {
						let vertices_start = vertices_idx;
						let indices_start = indices_idx;
						for vertex in mesh.vertices {
							vertices.write_offset(vertices_idx, Vertex::from(vertex));
							vertices_idx += 1;
						}
						for index in mesh.indices {
							indices.write_offset(indices_idx, index + vertices_start as u32);
							indices_idx += 1;
						}

						draw_cmds.push(DrawCmd {
							clip_rect: p.clip_rect,
							texture_id: mesh.texture_id,
							indices_offset: indices_start as u32,
							indices_count: (indices_idx - indices_start) as u32,
						})
					}
					Primitive::Callback(_) => (),
				}
			}

			assert_eq!(vertices_idx, vertex_cnt);
			assert_eq!(indices_idx, index_cnt);
		}

		// Safety: these buffers have never been used in any cmd
		let vertices = unsafe { Some(vertices.into_shared_unchecked()) };
		let indices = unsafe { Some(indices.into_shared_unchecked()) };

		Ok(EguiRenderOutput {
			render_ctx: self,
			vertices,
			indices,
			draw_cmds,
			render_scale: 1.,
		})
	}
}

/// See [`EguiRenderOutput::draw`]
pub struct EguiRenderOutput<'a, P: EguiBindlessPlatform> {
	render_ctx: &'a EguiRenderContext<P>,
	/// must be Some if `!draw_cmds.is_empty()`
	vertices: Option<RCDesc<P, Buffer<[Vertex]>>>,
	/// must be Some if `!draw_cmds.is_empty()`
	indices: Option<RCDesc<P, Buffer<[u32]>>>,
	draw_cmds: Vec<DrawCmd>,
	render_scale: f32,
}

impl<'a, P: EguiBindlessPlatform> EguiRenderOutput<'a, P> {
	fn empty(render_ctx: &'a EguiRenderContext<P>) -> Self {
		EguiRenderOutput {
			render_ctx,
			vertices: None,
			indices: None,
			draw_cmds: Vec::new(),
			render_scale: 1.,
		}
	}
}

struct DrawCmd {
	clip_rect: Rect,
	texture_id: TextureId,
	indices_offset: u32,
	indices_count: u32,
}

#[derive(Debug, Clone)]
pub struct EguiRenderingOptions {
	pub image_rt_load_op: LoadOp,
	pub depth_rt_load_op: LoadOp,
	pub render_scale: f32,
}

impl Default for EguiRenderingOptions {
	fn default() -> Self {
		Self {
			image_rt_load_op: LoadOp::Load,
			depth_rt_load_op: LoadOp::Load,
			render_scale: 1.0,
		}
	}
}

impl<P: EguiBindlessPlatform> EguiRenderOutput<'_, P> {
	/// render scale for window platforms
	pub(crate) fn render_scale(&mut self, scale: f32) {
		self.render_scale *= scale;
	}

	/// Draw the geometry of the ui onto some `color` and `depth` images. You may call this method multiple times with
	/// potentially a different [`EguiRenderPipeline`] to draw the ui multiple times, or don't call it at all. You must
	/// drop this struct to regain access to [`EguiRenderContext`] and further update the ui.
	///
	/// The `color` and `depth` images must match what was declared when creating the [`EguiRenderPipeline`], e.g. be
	/// either `Some` and be of the declared format or `None` if no image is expected, or an error may be returned. The
	/// images must be in [`ColorAttachment`] / [`DepthStencilAttachment`] layout respectively.
	pub fn draw<'a>(
		&self,
		pipeline: &EguiRenderPipeline<P>,
		cmd: &mut Recording<'a, P>,
		color: Option<&mut MutImageAccess<'a, P, Image2d, ColorAttachment>>,
		depth: Option<&mut MutImageAccess<'a, P, Image2d, DepthStencilAttachment>>,
		options: EguiRenderingOptions,
	) -> Result<(), EguiRenderingError<P>> {
		profiling::function_scope!();
		if !Arc::ptr_eq(&self.render_ctx.renderer.0, &pipeline.renderer.0) {
			return Err(EguiRenderingError::DifferentEguiRenderer);
		}

		let color = check_format_mismatch(pipeline.color_format, color, RenderTarget::Color)?;
		let depth = check_format_mismatch(pipeline.depth_format, depth, RenderTarget::Depth)?;
		let extent = match (color.as_ref().map(|c| &c.1), depth.as_ref().map(|c| &c.1)) {
			(Some(c), Some(d)) => {
				if c.extent() == d.extent() {
					Ok(c.extent())
				} else {
					Err(EguiRenderingError::MismatchExtent {
						color: c.extent(),
						depth: d.extent(),
					})
				}
			}
			(None, Some(d)) => Ok(d.extent()),
			(Some(c), None) => Ok(c.extent()),
			(None, None) => unreachable!(),
		}?;

		{
			let mut upload_wait = self.render_ctx.upload_wait.lock();
			upload_wait.push(cmd.resource_context().to_pending_execution());
		}

		if self.draw_cmds.is_empty() {
			return Ok(());
		}
		// must be present if draw_cmds are
		let indices = self.indices.as_ref().unwrap();
		let vertices = self.vertices.as_ref().unwrap().to_transient(cmd);

		let render_scale = options.render_scale * self.render_scale;
		cmd.begin_rendering(
			pipeline.render_pass_format(),
			color
				.map(|(_, color)| RenderingAttachment {
					image: color,
					load_op: options.image_rt_load_op,
					store_op: StoreOp::Store,
				})
				.as_slice(),
			depth.map(|(_, depth)| RenderingAttachment {
				image: depth,
				load_op: options.depth_rt_load_op,
				store_op: StoreOp::Store,
			}),
			|rp| {
				for draw in &self.draw_cmds {
					let texture = self
						.render_ctx
						.textures
						.get(&draw.texture_id)
						.ok_or(EguiRenderingError::<P>::MissingTexture(draw.texture_id))
						.unwrap();

					let rect = draw.clip_rect / render_scale;
					let rect = IRect2 {
						origin: IVec2::new(rect.min.x.floor() as i32, rect.min.y.floor() as i32),
						extent: UVec2::new(rect.width().ceil() as u32, rect.height().ceil() as u32),
					};

					rp.set_scissor(rect);
					rp.draw_indexed(
						&pipeline.graphics_pipeline,
						indices,
						DrawIndexedIndirectCommand {
							index_count: draw.indices_count,
							instance_count: 1,
							first_index: draw.indices_offset,
							vertex_offset: 0,
							first_instance: 0,
						},
						Param {
							screen_size_recip: (UVec2::from(extent).as_vec2() * render_scale).recip(),
							vertices,
							flags: ParamFlags::SRGB_FRAMEBUFFER | texture.texture_type.to_vertex_flags(),
							image: unsafe { UnsafeDesc::<Image<Image2d>>::new(texture.image.id()) },
							sampler: texture.sampler.to_strong(),
						},
					)?;
				}
				Ok(())
			},
		)?;

		Ok(())
	}
}

#[allow(clippy::type_complexity)]
fn check_format_mismatch<'a, 'b, P: EguiBindlessPlatform, A: ImageAccessType>(
	format: Option<Format>,
	image: Option<&'a mut MutImageAccess<'b, P, Image2d, A>>,
	rt: RenderTarget,
) -> Result<Option<(Format, &'a mut MutImageAccess<'b, P, Image2d, A>)>, EguiRenderingError<P>> {
	match (format, image) {
		(Some(format), Some(image)) => {
			if format == image.format() {
				Ok(Some((format, image)))
			} else {
				Err(EguiRenderingError::MismatchRTFormat {
					rt,
					expected: format,
					actual: image.format(),
				})
			}
		}
		(None, None) => Ok(None),
		(None, Some(_)) => Err(EguiRenderingError::UnexpectedRT(rt)),
		(Some(format), None) => Err(EguiRenderingError::ExpectedRT(rt, format)),
	}
}

#[derive(Copy, Clone, Debug)]
pub enum RenderTarget {
	Color,
	Depth,
}

#[derive(Error)]
pub enum EguiRenderingError<P: EguiBindlessPlatform> {
	#[error("Trying to render with a Pipeline and Context of differing `EguiRenderer`")]
	DifferentEguiRenderer,
	#[error("Expected {0:?} RT in format {1:?}, but got None")]
	ExpectedRT(RenderTarget, Format),
	#[error("Expected no {0:?} RT, but got some image")]
	UnexpectedRT(RenderTarget),
	#[error("Expected {rt:?} RT of format {expected:?}, but got an image of format {actual:?}")]
	MismatchRTFormat {
		rt: RenderTarget,
		expected: Format,
		actual: Format,
	},
	#[error("Color RT with extent {color:?} must match depth RT with extent {depth:?}")]
	MismatchExtent { color: Extent, depth: Extent },
	#[error(
		"TextureId {0:?} does not exist in the rendering backend. Has this `EguiRenderContext` been used with multiple egui `Context`s?"
	)]
	MissingTexture(TextureId),
	#[error("Recording Error: {0}")]
	RecordingError(#[from] RecordingError<P>),
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
