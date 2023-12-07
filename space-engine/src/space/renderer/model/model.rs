use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::Sharing;

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::model::model_descriptor_set::{ModelDescriptorSet, ModelDescriptorSetLayout};
use crate::space::Init;

pub struct OpaqueModel {
	pub vertex_data: Subbuffer<[ModelVertex]>,
	pub descriptor: ModelDescriptorSet,
}

impl OpaqueModel {
	pub fn new<I>(init: &Arc<Init>, model_descriptor_set_layout: &ModelDescriptorSetLayout, vertex_input: I) -> Self
	where
		I: IntoIterator<Item = ModelVertex>,
		I::IntoIter: ExactSizeIterator,
	{
		let vertex_data = Buffer::from_iter(
			init.memory_allocator.clone(),
			BufferCreateInfo {
				usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
				sharing: Sharing::Exclusive,
				..BufferCreateInfo::default()
			},
			AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..AllocationCreateInfo::default()
			},
			vertex_input,
		)
		.unwrap();
		let descriptor = ModelDescriptorSet::new(init, model_descriptor_set_layout, vertex_data.clone());
		Self {
			vertex_data,
			descriptor,
		}
	}
}
