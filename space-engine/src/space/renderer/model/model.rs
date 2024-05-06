use crate::space::renderer::model::texture_manager::TextureManager;
use space_engine_common::space::renderer::model::model_vertex::ModelVertex;
use std::sync::Arc;
use vulkano::buffer::BufferUsage;
use vulkano_bindless::descriptor::buffer::Buffer;
use vulkano_bindless::descriptor::rc_reference::RCDesc;
use vulkano_bindless::descriptor::SampledImage2D;

pub struct OpaqueModel {
	pub vertex_buffer: RCDesc<Buffer<[ModelVertex]>>,
	pub index_buffer: RCDesc<Buffer<[u32]>>,
	pub strong_refs: Vec<RCDesc<SampledImage2D>>,
}

impl OpaqueModel {
	pub async fn direct<V>(
		texture_manager: &Arc<TextureManager>,
		vertex_data: V,
		strong_refs: impl IntoIterator<Item = RCDesc<SampledImage2D>>,
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
		Self {
			vertex_buffer,
			index_buffer,
			strong_refs: strong_refs.into_iter().collect(),
		}
	}

	pub async fn indexed<I, V>(
		texture_manager: &Arc<TextureManager>,
		index_data: I,
		vertex_data: V,
		strong_refs: impl IntoIterator<Item = RCDesc<SampledImage2D>>,
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
		Self {
			vertex_buffer,
			index_buffer,
			strong_refs: strong_refs.into_iter().collect(),
		}
	}
}
