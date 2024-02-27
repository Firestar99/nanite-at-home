use std::ops::Deref;
use std::sync::Arc;

use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayout, DescriptorType};
use vulkano::descriptor_set::{DescriptorImageViewInfo, DescriptorSet, WriteDescriptorSet};
use vulkano::image::view::ImageView;
use vulkano::image::ImageLayout;
use vulkano::shader::ShaderStages;

use crate::space::renderer::descriptor_set_creator::DescriptorSetBinding;
use crate::space::Init;

#[derive(Clone)]
pub struct TextureArrayDescriptorSetLayout(pub Arc<DescriptorSetLayout>);

impl TextureArrayDescriptorSetLayout {
	pub const SHADER_STAGES: ShaderStages = ShaderStages::all_graphics().union(ShaderStages::COMPUTE);

	pub fn binding_model_texture(_init: &Arc<Init>) -> DescriptorSetBinding {
		DescriptorSetBinding {
			flags: DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT,
			// TODO StandardDescriptorSetAllocator pre-allocates 32 * this number many descriptors by default, which is waaay too many, and runs out of device memory
			// descriptor_count: _init
			// 	.device
			// 	.physical_device()
			// 	.properties()
			// 	.max_descriptor_set_sampled_images,
			descriptor_count: 4096,
			..DescriptorSetBinding::descriptor_type(0, DescriptorType::SampledImage)
		}
	}

	pub fn new(init: &Arc<Init>) -> Self {
		Self(
			DescriptorSetBinding::create_descriptor_set_layout(
				&[&Self::binding_model_texture(init)],
				init,
				Self::SHADER_STAGES,
			)
			.unwrap(),
		)
	}
}

impl Deref for TextureArrayDescriptorSetLayout {
	type Target = Arc<DescriptorSetLayout>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Clone)]
pub struct TextureArrayDescriptorSet(pub Arc<DescriptorSet>);

impl TextureArrayDescriptorSet {
	pub fn new(init: &Arc<Init>, layout: &TextureArrayDescriptorSetLayout, images: &[Arc<ImageView>]) -> Self {
		Self(
			DescriptorSet::new_variable(
				init.descriptor_allocator.clone(),
				layout.deref().clone(),
				images.len() as u32,
				[WriteDescriptorSet::image_view_with_layout_array(
					*TextureArrayDescriptorSetLayout::binding_model_texture(init),
					0,
					images.iter().map(|image_view| DescriptorImageViewInfo {
						image_view: image_view.clone(),
						image_layout: ImageLayout::ShaderReadOnlyOptimal,
					}),
				)],
				[],
			)
			.unwrap(),
		)
	}

	pub fn layout(&self) -> TextureArrayDescriptorSetLayout {
		TextureArrayDescriptorSetLayout(self.0.layout().clone())
	}
}

impl Deref for TextureArrayDescriptorSet {
	type Target = Arc<DescriptorSet>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
