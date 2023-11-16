use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::Sharing;

use space_engine_common::space::renderer::lod_obj::VertexInput;

use crate::space::renderer::lod_obj::opaque_draw::OpaqueDrawPipeline;
use crate::space::renderer::render_graph::context::RenderContext;

pub struct OpaqueModel {
	pub vertex_input_buffer: Subbuffer<[VertexInput]>,
	pub descriptor: Arc<PersistentDescriptorSet>,
}

impl OpaqueModel {
	pub fn new<I>(context: &Arc<RenderContext>, opaque_draw_pipeline: &OpaqueDrawPipeline, vertex_input: I) -> Self
	where
		I: IntoIterator<Item = VertexInput>,
		I::IntoIter: ExactSizeIterator,
	{
		let init = &context.init;
		let vertex_input_buffer = Buffer::from_iter(
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
		let descriptor = PersistentDescriptorSet::new(
			&init.descriptor_allocator,
			opaque_draw_pipeline.descriptor_set_layout_model().clone(),
			[WriteDescriptorSet::buffer(0, vertex_input_buffer.clone())],
			[],
		)
		.unwrap();
		Self {
			vertex_input_buffer,
			descriptor,
		}
	}
}
