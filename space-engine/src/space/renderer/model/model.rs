use std::sync::Arc;

use vulkano::buffer::{BufferUsage, Subbuffer};

use space_engine_common::space::renderer::model::model_vertex::ModelVertex;

use crate::space::renderer::model::model_descriptor_set::{ModelDescriptorSet, ModelDescriptorSetLayout};
use crate::space::renderer::model::texture_manager::TextureManager;
use crate::space::Init;

pub struct OpaqueModel {
	pub vertex_buffer: Subbuffer<[ModelVertex]>,
	pub index_buffer: Subbuffer<[u32]>,
	pub descriptor: ModelDescriptorSet,
}

impl OpaqueModel {
	pub async fn direct<V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		vertex_data: V,
	) -> Self
	where
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let vertex_data = vertex_data.into_iter();
		let vertex_len = vertex_data.len() as u32;
		let vertex_buffer =
			texture_manager.upload_buffer(BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST, vertex_data);
		let index_buffer =
			texture_manager.upload_buffer(BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST, 0..vertex_len);
		Self::new(
			init,
			texture_manager,
			model_descriptor_set_layout,
			vertex_buffer,
			index_buffer,
		)
	}

	pub async fn indexed<I, V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		index_data: I,
		vertex_data: V,
	) -> Self
	where
		I: IntoIterator<Item = u32>,
		I::IntoIter: ExactSizeIterator,
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let vertex_buffer =
			texture_manager.upload_buffer(BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST, vertex_data);
		let index_buffer =
			texture_manager.upload_buffer(BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST, index_data);
		Self::new(
			init,
			texture_manager,
			model_descriptor_set_layout,
			vertex_buffer,
			index_buffer,
		)
	}

	fn new(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		vertex_buffer: Subbuffer<[ModelVertex]>,
		index_buffer: Subbuffer<[u32]>,
	) -> Self {
		let descriptor = ModelDescriptorSet::new(
			init,
			model_descriptor_set_layout,
			&vertex_buffer,
			&index_buffer,
			&texture_manager.sampler,
		);
		Self {
			vertex_buffer,
			index_buffer,
			descriptor,
		}
	}
}
