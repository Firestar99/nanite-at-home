use std::ops::Deref;
use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::descriptor_set::layout::{DescriptorSetLayout, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::image::sampler::Sampler;
use vulkano::shader::ShaderStages;

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::descriptor_set_creator::DescriptorSetBinding;
use crate::space::Init;

#[derive(Clone)]
pub struct ModelDescriptorSetLayout(pub Arc<DescriptorSetLayout>);

impl ModelDescriptorSetLayout {
	pub const SHADER_STAGES: ShaderStages = ShaderStages::all_graphics().union(ShaderStages::COMPUTE);
	pub const BINDING_MODEL_VERTEX: DescriptorSetBinding =
		DescriptorSetBinding::descriptor_type(0, DescriptorType::StorageBuffer);
	pub const BINDING_MODEL_SAMPLER: DescriptorSetBinding =
		DescriptorSetBinding::descriptor_type(1, DescriptorType::Sampler);

	pub fn new(init: &Arc<Init>) -> Self {
		Self(
			DescriptorSetBinding::create_descriptor_set_layout(
				&[&Self::BINDING_MODEL_VERTEX, &Self::BINDING_MODEL_SAMPLER],
				init,
				Self::SHADER_STAGES,
			)
			.unwrap(),
		)
	}
}

impl Deref for ModelDescriptorSetLayout {
	type Target = Arc<DescriptorSetLayout>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Clone)]
pub struct ModelDescriptorSet(pub Arc<DescriptorSet>);

impl ModelDescriptorSet {
	pub fn new(
		init: &Arc<Init>,
		layout: &ModelDescriptorSetLayout,
		vertex_data: &Subbuffer<[ModelVertex]>,
		sampler: &Arc<Sampler>,
	) -> Self {
		Self(
			DescriptorSet::new(
				init.descriptor_allocator.clone(),
				layout.0.clone(),
				[
					WriteDescriptorSet::buffer(*ModelDescriptorSetLayout::BINDING_MODEL_VERTEX, vertex_data.clone()),
					WriteDescriptorSet::sampler(*ModelDescriptorSetLayout::BINDING_MODEL_SAMPLER, sampler.clone()),
				],
				[],
			)
			.unwrap(),
		)
	}

	pub fn layout(&self) -> ModelDescriptorSetLayout {
		ModelDescriptorSetLayout(self.0.layout().clone())
	}
}

impl Deref for ModelDescriptorSet {
	type Target = Arc<DescriptorSet>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
