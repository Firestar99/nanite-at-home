use glam::{Vec2, Vec4};
use rust_gpu_bindless_macros::{BufferStruct, BufferStructPlain, bindless};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[derive(Copy, Clone, BufferStructPlain)]
pub struct Vertex {
	pub position: Vec2,
	pub color: Vec4,
}

impl Vertex {
	pub fn new(position: Vec2, color: Vec4) -> Self {
		Self { position, color }
	}
}

#[derive(Copy, Clone, BufferStruct)]
pub struct Param<'a> {
	pub vertices: TransientDesc<'a, Buffer<[Vertex]>>,
}

#[bindless(vertex())]
pub fn triangle_vertex(
	#[bindless(descriptors)] descriptors: Descriptors<'_>,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(vertex_index)] vertex_index: u32,
	#[spirv(position)] out_position: &mut Vec4,
	vertex_color: &mut Vec4,
) {
	let vertex = param.vertices.access(&descriptors).load(vertex_index as usize);
	*out_position = Vec4::from((vertex.position, 0., 1.));
	*vertex_color = vertex.color;
}

#[bindless(fragment())]
pub fn triangle_fragment(
	// #[bindless(descriptors)] descriptors: Descriptors<'_>,
	#[bindless(param)] _param: &Param<'static>,
	vertex_color: Vec4,
	out_color: &mut Vec4,
) {
	*out_color = vertex_color;
}
