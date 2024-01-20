use std::sync::Arc;

use vulkano::buffer::{BufferUsage, Subbuffer};

use space_engine_common::space::renderer::model::model_vertex::{ModelTextureId, ModelVertex};

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
		albedo_tex_id: ModelTextureId,
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
			albedo_tex_id,
		)
		.await
	}

	pub async fn indexed<I, V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		index_data: I,
		vertex_data: V,
		albedo_tex_id: ModelTextureId,
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
			albedo_tex_id,
		)
		.await
	}

	async fn new<I, V>(
		init: &Arc<Init>,
		texture_manager: &Arc<TextureManager>,
		model_descriptor_set_layout: &ModelDescriptorSetLayout,
		index_data: Option<I>,
		vertex_data: V,
		albedo_tex_id: ModelTextureId,
	) -> Self
	where
		I: IntoIterator<Item = u16>,
		I::IntoIter: ExactSizeIterator,
		V: IntoIterator<Item = ModelVertex>,
		V::IntoIter: ExactSizeIterator,
	{
		let vertex_buffer = texture_manager.upload_buffer(
			BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
			vertex_data.into_iter().map(|vtx| ModelVertex {
				tex_id: albedo_tex_id,
				..vtx
			}),
		);

		let index_buffer = index_data.map(|index_data| {
			texture_manager.upload_buffer(BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST, index_data)
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
