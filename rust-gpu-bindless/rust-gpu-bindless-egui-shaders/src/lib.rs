#![no_std]

pub use crate::vertex::*;
use bitflags::bitflags;
use core::fmt;
use glam::{Vec2, Vec4};
use rust_gpu_bindless_macros::{bindless, BufferStruct};
use rust_gpu_bindless_shaders::descriptor::{Buffer, Descriptors, Image, Image2d, Sampler, StrongDesc, TransientDesc};
pub use rust_gpu_bindless_shaders::utils::srgb::*;

mod vertex;

#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct ImageVertex {
	pub vertex: Vertex,
	pub image: StrongDesc<Image<Image2d>>,
	pub sampler: StrongDesc<Sampler>,
}

#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct Param<'a> {
	pub vertices: TransientDesc<'a, Buffer<[ImageVertex]>>,
	pub flags: ParamFlags,
}

#[derive(Clone, Copy, BufferStruct)]
pub struct ParamFlags(u32);

bitflags! {
	impl ParamFlags: u32 {
		const SRGB_FRAMEBUFFER = 1;
	}
}

impl fmt::Debug for ParamFlags {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		bitflags::parser::to_writer(self, f)
	}
}

#[bindless(vertex())]
pub fn egui_vertex(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	#[spirv(vertex_index)] vertex_id: u32,
	#[spirv(position)] position: &mut Vec4,
	vtx_color_srgb: &mut Vec4,
	vtx_uv: &mut Vec2,
	image: &mut StrongDesc<Image<Image2d>>,
	sampler: &mut StrongDesc<Sampler>,
) {
	let vertex = param.vertices.access(&descriptors).load(vertex_id as usize);
	*position = Vec4::from((vertex.vertex.pos, 1., 1.));
	*vtx_color_srgb = vertex.vertex.color();
	*vtx_uv = vertex.vertex.uv;
	*image = vertex.image;
	*sampler = vertex.sampler;
}

#[bindless(fragment())]
pub fn egui_fragment(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	vtx_color_srgb: Vec4,
	vtx_uv: Vec2,
	#[spirv(flat)] image: StrongDesc<Image<Image2d>>,
	#[spirv(flat)] sampler: StrongDesc<Sampler>,
	frag_color: &mut Vec4,
) {
	// our image is unorm, but the values stored inside are in srgb colorspace
	// this sample op should not do any srgb conversions
	let image_srgb: Vec4 = image.access(&descriptors).sample(sampler.access(&descriptors), vtx_uv);
	let out_color_srgb = vtx_color_srgb * image_srgb;

	*frag_color = if param.flags.contains(ParamFlags::SRGB_FRAMEBUFFER) {
		out_color_srgb
	} else {
		linear_to_srgb_alpha(out_color_srgb)
	}
}
