use crate::space::renderer::model::model_vertex::ModelVertex;
use bytemuck_derive::AnyBitPattern;
use vulkano_bindless_shaders::descriptor::{Buffer, WeakDesc};

#[repr(C)]
#[derive(Copy, Clone, AnyBitPattern)]
pub struct OpaqueGpuModel {
	pub vertex_buffer: WeakDesc<Buffer<[ModelVertex]>>,
	pub index_buffer: WeakDesc<Buffer<[u32]>>,
	pub triangle_count: u32,
}
