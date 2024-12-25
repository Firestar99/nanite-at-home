use core::fmt::{Debug, Formatter};
use glam::{Vec2, Vec4};
use num_enum::{FromPrimitive, IntoPrimitive};
use rust_gpu_bindless_macros::{bindless, BufferStruct, BufferStructPlain};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, TransientDesc};

#[repr(u32)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Hash, FromPrimitive, IntoPrimitive)]
pub enum Color {
	Red,
	Cyan,
	Yellow,
	Black,
	#[default]
	Unknown,
}

impl Debug for Color {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		match self {
			Color::Red => f.write_str("R"),
			Color::Cyan => f.write_str("C"),
			Color::Yellow => f.write_str("Y"),
			Color::Black => f.write_str("B"),
			Color::Unknown => f.write_str("U"),
		}
	}
}

impl Color {
	const COLORS: &'static [Vec4] = &[
		Vec4::new(1., 0., 0., 1.),
		Vec4::new(0., 1., 1., 1.),
		Vec4::new(1., 1., 0., 1.),
		Vec4::new(0., 0., 0., 0.),
		Vec4::new(1., 1., 1., 1.),
	];

	pub fn parse(color: Vec4) -> Self {
		for (i, value) in Self::COLORS.iter().enumerate() {
			if (color - value).length() <= 0.01 {
				return Self::from_primitive(i as u32);
			}
		}
		Self::Unknown
	}

	pub fn color(&self) -> Vec4 {
		Self::COLORS[u32::from(*self) as usize]
	}
}

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
