use crate::space::renderer::model::model_vertex::ModelVertex;
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::reference::StrongDesc;
use vulkano_bindless_shaders::descriptor::Buffer;

#[repr(C)]
#[derive(Copy, Clone, DescStruct)]
pub struct OpaqueGpuModel<'a> {
	pub vertex_buffer: StrongDesc<'a, Buffer<[ModelVertex<'static>]>>,
	pub index_buffer: StrongDesc<'a, Buffer<[u32]>>,
	pub triangle_count: u32,
}
