use crate::space::renderer::model::model_vertex::ModelVertex;
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::Buffer;

#[repr(C)]
#[derive(Copy, Clone, DescStruct)]
pub struct OpaqueGpuModel {
	pub vertex_buffer: StrongDesc<Buffer<[ModelVertex]>>,
	pub index_buffer: StrongDesc<Buffer<[u32]>>,
	pub triangle_count: u32,
}
