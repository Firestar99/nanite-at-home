use std::ops::Deref;
use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::layout::DescriptorType::UniformBuffer;
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::shader::ShaderStages;

use space_engine_common::space::renderer::frame_data::FrameData;

use crate::space::renderer::descriptor_set_creator::DescriptorSetBinding;
use crate::space::Init;

#[derive(Clone)]
pub struct GlobalDescriptorSetLayout(pub Arc<DescriptorSetLayout>);

impl GlobalDescriptorSetLayout {
	pub const SHADER_STAGES: ShaderStages = ShaderStages::all_graphics().union(ShaderStages::COMPUTE);
	pub const BINDING_FRAME_DATA: DescriptorSetBinding = DescriptorSetBinding::descriptor_type(0, UniformBuffer);

	pub fn new(init: &Arc<Init>) -> Self {
		Self(
			DescriptorSetBinding::create_descriptor_set_layout(&[&Self::BINDING_FRAME_DATA], init, Self::SHADER_STAGES)
				.unwrap(),
		)
	}
}

impl Deref for GlobalDescriptorSetLayout {
	type Target = Arc<DescriptorSetLayout>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Clone)]
pub struct GlobalDescriptorSet(pub Arc<DescriptorSet>);

impl GlobalDescriptorSet {
	pub fn new(init: &Arc<Init>, layout: &GlobalDescriptorSetLayout, frame_data_uniform: Subbuffer<FrameData>) -> Self {
		GlobalDescriptorSet(
			DescriptorSet::new(
				init.descriptor_allocator.clone(),
				layout.0.clone(),
				[WriteDescriptorSet::buffer(0, frame_data_uniform)],
				[],
			)
			.unwrap(),
		)
	}

	pub fn layout(&self) -> GlobalDescriptorSetLayout {
		GlobalDescriptorSetLayout(self.0.layout().clone())
	}
}

impl Deref for GlobalDescriptorSet {
	type Target = Arc<DescriptorSet>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
