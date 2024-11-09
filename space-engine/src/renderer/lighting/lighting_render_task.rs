use crate::renderer::lighting::lighting_pipeline::LightingPipeline;
use crate::renderer::lighting::sky_shader_pipeline::SkyShaderPipeline;
use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::command_buffer::{CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer};
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::image::view::ImageView;
use vulkano::shader::ShaderStages;
use vulkano::sync::GpuFuture;

pub struct LightingRenderTask {
	init: Arc<Init>,
	image_descriptor_set_layout: Arc<DescriptorSetLayout>,
	pipeline_lighting: LightingPipeline,
	pipeline_sky_shader: SkyShaderPipeline,
}

impl LightingRenderTask {
	pub fn new(init: &Arc<Init>) -> Self {
		let image_descriptor_set_layout = DescriptorSetLayout::new(
			init.device.clone(),
			DescriptorSetLayoutCreateInfo {
				bindings: BTreeMap::from_iter(
					(0..4)
						.map(|i| {
							(
								i as u32,
								DescriptorSetLayoutBinding {
									stages: ShaderStages::COMPUTE,
									..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::SampledImage)
								},
							)
						})
						.chain([(
							4,
							DescriptorSetLayoutBinding {
								stages: ShaderStages::COMPUTE,
								..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageImage)
							},
						)]),
				),
				..DescriptorSetLayoutCreateInfo::default()
			},
		)
		.unwrap();
		let pipeline_lighting = LightingPipeline::new(init, &image_descriptor_set_layout);
		let pipeline_sky_shader = SkyShaderPipeline::new(init, &image_descriptor_set_layout);

		Self {
			init: init.clone(),
			image_descriptor_set_layout,
			pipeline_lighting,
			pipeline_sky_shader,
		}
	}

	#[allow(clippy::too_many_arguments)]
	#[profiling::function]
	pub fn record(
		&self,
		frame_context: &FrameContext,
		g_albedo: &Arc<ImageView>,
		g_normal: &Arc<ImageView>,
		g_roughness_metallic: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		output_image: &Arc<ImageView>,
		future: impl GpuFuture,
	) -> impl GpuFuture {
		let init = &self.init;
		let graphics = &init.queues.client.graphics_main;

		let image_descriptor = DescriptorSet::new(
			frame_context.render_context.init.descriptor_allocator.clone(),
			self.image_descriptor_set_layout.clone(),
			[
				WriteDescriptorSet::image_view(0, g_albedo.clone()),
				WriteDescriptorSet::image_view(1, g_normal.clone()),
				WriteDescriptorSet::image_view(2, g_roughness_metallic.clone()),
				WriteDescriptorSet::image_view(3, depth_image.clone()),
				WriteDescriptorSet::image_view(4, output_image.clone()),
			],
			[],
		)
		.unwrap();

		let mut cmd = RecordingCommandBuffer::new(
			init.cmd_buffer_allocator.clone(),
			graphics.queue_family_index(),
			CommandBufferLevel::Primary,
			CommandBufferBeginInfo {
				usage: CommandBufferUsage::OneTimeSubmit,
				..CommandBufferBeginInfo::default()
			},
		)
		.unwrap();
		self.pipeline_sky_shader
			.dispatch(frame_context, image_descriptor.clone(), &mut cmd);
		self.pipeline_lighting
			.dispatch(frame_context, image_descriptor, &mut cmd);
		let cmd = cmd.end().unwrap();

		future.then_execute(graphics.clone(), cmd).unwrap()
	}
}
