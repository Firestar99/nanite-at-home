use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::Sharing;

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::model::model_descriptor_set::{ModelDescriptorSet, ModelDescriptorSetLayout};
use crate::space::renderer::model::texture_manager::TextureManager;
use crate::space::Init;

pub struct OpaqueModel {
	pub vertex_buffer: Subbuffer<[ModelVertex]>,
	pub index_buffer: Option<Subbuffer<[u16]>>,
	pub descriptor: ModelDescriptorSet,
}

impl OpaqueModel {
	pub async fn direct<V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		vertex_data: V,
		image_data: &[u8],
	) -> Self
	where
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		Self::new::<Vec<u16>, V>(
			init,
			texture_manager,
			model_descriptor_set_layout,
			None,
			vertex_data,
			image_data,
		)
		.await
	}

	pub async fn indexed<I, V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		index_data: I,
		vertex_data: V,
		image_data: &[u8],
	) -> Self
	where
		I: IntoIterator<Item = u16>,
		I::IntoIter: ExactSizeIterator,
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		Self::new(
			init,
			texture_manager,
			model_descriptor_set_layout,
			Some(index_data),
			vertex_data,
			image_data,
		)
		.await
	}

	async fn new<I, V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		index_data: Option<I>,
		vertex_data: V,
		image_data: &[u8],
	) -> Self
	where
		I: IntoIterator<Item = u16>,
		I::IntoIter: ExactSizeIterator,
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let (_image_view, tex_id) = texture_manager.upload_texture_from_memory(image_data).await.unwrap();

		let vertex_buffer = Buffer::from_iter(
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
			vertex_data.into_iter().map(|vtx| ModelVertex { tex_id, ..vtx }),
		)
		.unwrap();

		let index_buffer = index_data.map(|index_data| {
			Buffer::from_iter(
				init.memory_allocator.clone(),
				BufferCreateInfo {
					usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
					sharing: Sharing::Exclusive,
					..BufferCreateInfo::default()
				},
				AllocationCreateInfo {
					memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
					..AllocationCreateInfo::default()
				},
				index_data.into_iter(),
			)
			.unwrap()
		});

		let descriptor = ModelDescriptorSet::new(
			init,
			model_descriptor_set_layout,
			&vertex_buffer,
			&texture_manager.sampler,
		);
		Self {
			vertex_buffer,
			index_buffer,
			descriptor,
		}
	}
}
