use crate::space::renderer::model::model_vertex::ModelVertex;
use vulkano_bindless_shaders::descriptor::buffer::Buffer;
use vulkano_bindless_shaders::descriptor::{Sampler, TransientDesc};

#[derive(Copy, Clone)]
pub struct PushConstant<'a> {
	pub vertex_buffer: TransientDesc<'a, Buffer<[ModelVertex]>>,
	pub index_buffer: TransientDesc<'a, Buffer<[u32]>>,
	pub sampler: TransientDesc<'a, Sampler>,
}

unsafe impl bytemuck::Zeroable for PushConstant<'static> {}

unsafe impl bytemuck::AnyBitPattern for PushConstant<'static> {}

impl<'a> PushConstant<'a> {
	pub unsafe fn to_static(&self) -> PushConstant<'static> {
		unsafe {
			PushConstant {
				vertex_buffer: self.vertex_buffer.to_static(),
				index_buffer: self.index_buffer.to_static(),
				sampler: self.sampler.to_static(),
			}
		}
	}
}
