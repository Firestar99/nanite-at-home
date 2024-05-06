use crate::space::Init;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{
	DescriptorBindingFlags, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
	DescriptorType,
};
use vulkano::image::sampler::Sampler;
use vulkano::shader::ShaderStages;
use vulkano::{Validated, VulkanError};

#[derive(Clone, Debug)]
pub struct DescriptorSetBinding {
	pub binding_id: u32,
	pub descriptor_type: DescriptorType,
	pub descriptor_count: u32,
	pub flags: DescriptorBindingFlags,
	pub immutable_samplers: Vec<Arc<Sampler>>,
}

impl DescriptorSetBinding {
	pub const fn descriptor_type(binding_id: u32, descriptor_type: DescriptorType) -> Self {
		Self {
			binding_id,
			descriptor_type,
			descriptor_count: 1,
			flags: DescriptorBindingFlags::empty(),
			immutable_samplers: Vec::new(),
		}
	}

	fn to_descriptor_set_layout_binding(self, stages: ShaderStages) -> (u32, DescriptorSetLayoutBinding) {
		(
			self.binding_id,
			DescriptorSetLayoutBinding {
				stages,
				binding_flags: self.flags,
				descriptor_count: self.descriptor_count,
				immutable_samplers: self.immutable_samplers,
				..DescriptorSetLayoutBinding::descriptor_type(self.descriptor_type)
			},
		)
	}

	pub fn create_descriptor_set_layout_create_info(
		binding: &[&Self],
		stages: ShaderStages,
	) -> DescriptorSetLayoutCreateInfo {
		DescriptorSetLayoutCreateInfo {
			bindings: binding
				.iter()
				.map(|b| (**b).clone().to_descriptor_set_layout_binding(stages))
				.collect(),
			..DescriptorSetLayoutCreateInfo::default()
		}
	}

	pub fn create_descriptor_set_layout(
		binding: &[&Self],
		init: &Arc<Init>,
		stages: ShaderStages,
	) -> Result<Arc<DescriptorSetLayout>, Validated<VulkanError>> {
		DescriptorSetLayout::new(
			init.device.clone(),
			Self::create_descriptor_set_layout_create_info(binding, stages),
		)
	}
}

impl Deref for DescriptorSetBinding {
	type Target = u32;

	fn deref(&self) -> &Self::Target {
		&self.binding_id
	}
}
