use crate::renderer::compacting_alloc_buffer::CompactingAllocBuffer;
use crate::renderer::frame_context::FrameContext;
use crate::renderer::lighting::lighting_compute::LightingCompute;
use crate::renderer::lighting::sky_shader_compute::SkyShaderCompute;
use crate::renderer::meshlet::instance_cull_compute::InstanceCullCompute;
use crate::renderer::meshlet::meshlet_draw::MeshletDraw;
use crate::renderer::meshlet::meshlet_select_compute::MeshletSelectCompute;
use anyhow::anyhow;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessImageCreateInfo, BindlessImageUsage, Extent, Format, Image2d, ImageDescExt, MutDesc, MutImage,
};
use rust_gpu_bindless::pipeline::{
	ClearValue, ColorAttachment, DepthStencilAttachment, ImageAccessType, LoadOp, MutImageAccess, MutImageAccessExt,
	Recording, RenderPassFormat, RenderingAttachment, SampledRead, StorageReadWrite, StoreOp,
};
use space_asset_rt::meshlet::scene::InstancedMeshletSceneCpu;
use space_engine_shader::renderer::frame_data::FrameData;
use space_engine_shader::renderer::g_buffer::GBuffer;
use space_engine_shader::renderer::meshlet::intermediate::{MeshletGroupInstance, MeshletInstance};
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
pub struct RenderPipelineMainFormat {
	pub output_format: Format,
	pub g_albedo_format: Format,
	pub g_normal_format: Format,
	pub g_rm_format: Format,
	pub depth_format: Format,
}

impl RenderPipelineMainFormat {
	pub fn to_g_buffer_rp(&self) -> RenderPassFormat {
		RenderPassFormat::new(
			&[self.g_albedo_format, self.g_normal_format, self.g_rm_format],
			Some(self.depth_format),
		)
	}
}

pub struct RenderPipelineMain {
	pub bindless: Bindless,
	pub format: RenderPipelineMainFormat,
	pub meshlet_group_capacity: usize,
	pub meshlet_instance_capacity: usize,
	pub instance_cull: InstanceCullCompute,
	pub meshlet_select: MeshletSelectCompute,
	pub meshlet_draw: MeshletDraw,
	pub lighting: LightingCompute,
	pub sky_shader: SkyShaderCompute,
}

impl RenderPipelineMain {
	pub fn new(
		bindless: &Bindless,
		output_format: Format,
		meshlet_group_capacity: usize,
		meshlet_instance_capacity: usize,
	) -> anyhow::Result<Arc<Self>> {
		// all formats are always available
		let format = RenderPipelineMainFormat {
			output_format,
			depth_format: Format::D32_SFLOAT,
			g_albedo_format: Format::R8G8B8A8_SRGB,
			g_normal_format: Format::R16G16B16A16_SFLOAT,
			g_rm_format: Format::R16G16_SFLOAT,
		};

		Ok(Arc::new(Self {
			bindless: bindless.clone(),
			format,
			meshlet_group_capacity,
			meshlet_instance_capacity,
			instance_cull: InstanceCullCompute::new(bindless)?,
			meshlet_select: MeshletSelectCompute::new(bindless)?,
			meshlet_draw: MeshletDraw::new(bindless, format.to_g_buffer_rp())?,
			lighting: LightingCompute::new(bindless)?,
			sky_shader: SkyShaderCompute::new(bindless)?,
		}))
	}

	pub fn new_renderer(self: &Arc<Self>) -> anyhow::Result<RendererMain> {
		RendererMain::new(self.clone())
	}
}

pub struct RendererMain {
	pub pipeline: Arc<RenderPipelineMain>,
	resources: Option<RendererMainResources>,
}

struct RendererMainResources {
	extent: Extent,
	g_albedo: MutDesc<MutImage<Image2d>>,
	g_normal: MutDesc<MutImage<Image2d>>,
	g_roughness_metallic: MutDesc<MutImage<Image2d>>,
	depth_image: MutDesc<MutImage<Image2d>>,
	compacting_meshlet_groups: CompactingAllocBuffer<MeshletGroupInstance>,
	compacting_meshlet_instances: CompactingAllocBuffer<MeshletInstance>,
}

impl RendererMainResources {
	pub fn new(pipeline: &Arc<RenderPipelineMain>, extent: Extent) -> anyhow::Result<Self> {
		let g_albedo = pipeline.bindless.image().alloc(&BindlessImageCreateInfo {
			format: pipeline.format.g_albedo_format,
			extent,
			usage: BindlessImageUsage::COLOR_ATTACHMENT | BindlessImageUsage::SAMPLED,
			name: "g_albedo",
			..Default::default()
		})?;
		let g_normal = pipeline.bindless.image().alloc(&BindlessImageCreateInfo {
			format: pipeline.format.g_normal_format,
			extent,
			usage: BindlessImageUsage::COLOR_ATTACHMENT | BindlessImageUsage::SAMPLED,
			name: "g_normal",
			..Default::default()
		})?;
		let g_roughness_metallic = pipeline.bindless.image().alloc(&BindlessImageCreateInfo {
			format: pipeline.format.g_rm_format,
			extent,
			usage: BindlessImageUsage::COLOR_ATTACHMENT | BindlessImageUsage::SAMPLED,
			name: "g_roughness_metallic",
			..Default::default()
		})?;
		let depth_image = pipeline.bindless.image().alloc(&BindlessImageCreateInfo {
			format: pipeline.format.depth_format,
			extent,
			usage: BindlessImageUsage::DEPTH_STENCIL_ATTACHMENT | BindlessImageUsage::SAMPLED,
			name: "g_depth",
			..Default::default()
		})?;
		let compacting_meshlet_groups = CompactingAllocBuffer::new(
			&pipeline.bindless,
			pipeline.meshlet_group_capacity,
			[0, 1, 1],
			"compacting_meshlet_groups",
		)?;
		let compacting_meshlet_instances = CompactingAllocBuffer::new(
			&pipeline.bindless,
			pipeline.meshlet_instance_capacity,
			[0, 1, 1],
			"compacting_meshlet_instances",
		)?;
		Ok(RendererMainResources {
			extent,
			depth_image,
			g_albedo,
			g_normal,
			g_roughness_metallic,
			compacting_meshlet_groups,
			compacting_meshlet_instances,
		})
	}
}

impl RendererMain {
	fn new(pipeline: Arc<RenderPipelineMain>) -> anyhow::Result<Self> {
		Ok(Self {
			pipeline,
			resources: None,
		})
	}

	pub fn new_frame(
		&mut self,
		cmd: &mut Recording<'_>,
		frame_data: FrameData,
		scene: &InstancedMeshletSceneCpu,
		output_image: &MutImageAccess<'_, Image2d, StorageReadWrite>,
	) -> anyhow::Result<()> {
		self.image_supported(output_image)?;
		let resources = {
			let extent = output_image.extent();
			let resources = if let Some(resources) = self.resources.take() {
				if resources.extent == extent {
					Some(resources)
				} else {
					drop(resources);
					None
				}
			} else {
				None
			};
			if let Some(resources) = resources {
				resources
			} else {
				RendererMainResources::new(&self.pipeline, extent)?
			}
		};
		let frame_context = FrameContext::new(cmd, frame_data)?;

		let meshlet_instances = resources.compacting_meshlet_instances.transition_writing(cmd)?;
		let meshlet_groups = resources.compacting_meshlet_groups.transition_writing(cmd)?;
		self.pipeline
			.instance_cull
			.dispatch(cmd, &frame_context, scene, &meshlet_groups)?;
		let meshlet_groups = meshlet_groups.transition_reading()?;
		self.pipeline
			.meshlet_select
			.dispatch(cmd, &frame_context, scene, &meshlet_groups, &meshlet_instances)?;

		let meshlet_instances = meshlet_instances.transition_reading()?;
		let mut g_albedo = resources.g_albedo.access_dont_care::<ColorAttachment>(&cmd)?;
		let mut g_normal = resources.g_normal.access_dont_care::<ColorAttachment>(&cmd)?;
		let mut g_roughness_metallic = resources
			.g_roughness_metallic
			.access_dont_care::<ColorAttachment>(&cmd)?;
		let mut depth_image = resources.depth_image.access_dont_care::<DepthStencilAttachment>(&cmd)?;
		cmd.begin_rendering(
			self.pipeline.format.to_g_buffer_rp(),
			&[
				RenderingAttachment {
					image: &mut g_albedo,
					load_op: LoadOp::Clear(ClearValue::ColorF([0., 0., 0., 0.])),
					store_op: StoreOp::Store,
				},
				RenderingAttachment {
					image: &mut g_normal,
					load_op: LoadOp::DontCare,
					store_op: StoreOp::Store,
				},
				RenderingAttachment {
					image: &mut g_roughness_metallic,
					load_op: LoadOp::DontCare,
					store_op: StoreOp::Store,
				},
			],
			Some(RenderingAttachment {
				image: &mut depth_image,
				load_op: LoadOp::Clear(ClearValue::DepthStencil { depth: 1., stencil: 0 }),
				store_op: StoreOp::Store,
			}),
			|rendering| {
				self.pipeline
					.meshlet_draw
					.draw(rendering, &frame_context, scene, &meshlet_instances)?;
				Ok(())
			},
		)?;

		let g_albedo = g_albedo.transition::<SampledRead>()?;
		let g_normal = g_normal.transition::<SampledRead>()?;
		let g_roughness_metallic = g_roughness_metallic.transition::<SampledRead>()?;
		let depth_image = depth_image.transition::<SampledRead>()?;
		let g_buffer = GBuffer {
			g_albedo: g_albedo.to_transient_sampled()?,
			g_normal: g_normal.to_transient_sampled()?,
			g_roughness_metallic: g_roughness_metallic.to_transient_sampled()?,
			depth_image: depth_image.to_transient_sampled()?,
		};
		self.pipeline
			.sky_shader
			.dispatch(cmd, &frame_context, g_buffer, output_image)?;
		self.pipeline
			.lighting
			.dispatch(cmd, &frame_context, g_buffer, output_image)?;

		self.resources = Some(RendererMainResources {
			extent: resources.extent,
			g_albedo: g_albedo.into_desc(),
			g_normal: g_normal.into_desc(),
			g_roughness_metallic: g_roughness_metallic.into_desc(),
			depth_image: depth_image.into_desc(),
			compacting_meshlet_groups: meshlet_groups.transition_reset(),
			compacting_meshlet_instances: meshlet_instances.transition_reset(),
		});
		Ok(())
	}

	pub fn image_supported(&self, output_image: &MutImageAccess<Image2d, impl ImageAccessType>) -> anyhow::Result<()> {
		let extent = output_image.extent();
		if output_image.format() != self.pipeline.format.output_format {
			Err(anyhow!(
				"Expected format {:?} but output_image has format {:?}",
				self.pipeline.format.output_format,
				output_image.format()
			))
		} else if extent.depth != 1 {
			Err(anyhow!("Image was not 2D"))
		} else {
			Ok(())
		}
	}
}
