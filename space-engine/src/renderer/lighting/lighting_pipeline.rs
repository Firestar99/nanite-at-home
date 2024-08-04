use crate::renderer::render_graph::context::FrameContext;
use crate::renderer::Init;
use space_engine_shader::renderer::lighting::lighting_compute::{Params, LIGHTING_WG_SIZE};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::command_buffer::RecordingCommandBuffer;
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::image::view::ImageView;
use vulkano::pipeline::layout::PipelineLayoutCreateInfo;
use vulkano::pipeline::PipelineBindPoint;
use vulkano::pipeline::{Pipeline, PipelineLayout};
use vulkano::shader::ShaderStages;
use vulkano_bindless::pipeline::compute_pipeline::BindlessComputePipeline;

pub struct LightingPipeline {
	pipeline: BindlessComputePipeline<Params<'static>>,
	descriptor_set_1_layout: Arc<DescriptorSetLayout>,
}

impl LightingPipeline {
	pub fn new(init: &Arc<Init>) -> Self {
		let descriptor_set_1_layout = DescriptorSetLayout::new(
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

		let pipeline = BindlessComputePipeline::new(
			init.bindless.clone(),
			crate::shader::renderer::lighting::lighting_compute::lighting::new(),
			Some(init.pipeline_cache.deref().clone()),
			Some(
				PipelineLayout::new(
					init.device.clone(),
					PipelineLayoutCreateInfo {
						set_layouts: Vec::from([
							init.bindless.descriptor_set_layout.clone(),
							descriptor_set_1_layout.clone(),
						]),
						push_constant_ranges: init.bindless.get_push_constant::<Params<'static>>(),
						..PipelineLayoutCreateInfo::default()
					},
				)
				.unwrap(),
			),
		)
		.unwrap();

		Self {
			pipeline,
			descriptor_set_1_layout,
		}
	}

	#[profiling::function]
	pub fn dispatch(
		&self,
		frame_context: &FrameContext,
		g_albedo: &Arc<ImageView>,
		g_normal: &Arc<ImageView>,
		g_rm: &Arc<ImageView>,
		depth_image: &Arc<ImageView>,
		output_image: &Arc<ImageView>,
		cmd: &mut RecordingCommandBuffer,
	) {
		unsafe {
			let image_descriptor = DescriptorSet::new(
				frame_context.render_context.init.descriptor_allocator.clone(),
				self.descriptor_set_1_layout.clone(),
				[
					WriteDescriptorSet::image_view(0, g_albedo.clone()),
					WriteDescriptorSet::image_view(1, g_normal.clone()),
					WriteDescriptorSet::image_view(2, g_rm.clone()),
					WriteDescriptorSet::image_view(3, depth_image.clone()),
					WriteDescriptorSet::image_view(4, output_image.clone()),
				],
				[],
			)
			.unwrap();

			let image_size = frame_context.frame_data.viewport_size;
			let groups = [
				(image_size.x + LIGHTING_WG_SIZE - 1) / LIGHTING_WG_SIZE,
				image_size.y,
				1,
			];
			self.pipeline
				.dispatch(
					cmd,
					groups,
					|cmd| {
						cmd.bind_descriptor_sets(
							PipelineBindPoint::Compute,
							self.pipeline.layout().clone(),
							1,
							image_descriptor,
						)
					},
					Params {
						frame_data: frame_context.frame_data_desc,
					},
				)
				.unwrap();
		}
	}
}
