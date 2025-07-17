#![no_std]

pub use crate::vertex::*;
use bitflags::bitflags;
use core::fmt;
use glam::{Vec2, Vec4, Vec4Swizzles};
use rust_gpu_bindless_macros::{BufferStruct, bindless};
use rust_gpu_bindless_shaders::descriptor::{
	Buffer, Descriptors, Image, Image2d, Sampler, StrongDesc, TransientDesc, UnsafeDesc,
};
pub use rust_gpu_bindless_shaders::utils::srgb::*;

mod vertex;

/// Usually, we would use StrongDesc to reference the image and sampler, but they need to be mutable and we cannot turn
/// mutable images
#[derive(Copy, Clone, Debug, BufferStruct)]
pub struct Param<'a> {
	pub vertices: TransientDesc<'a, Buffer<[Vertex]>>,
	pub image: UnsafeDesc<Image<Image2d>>,
	pub sampler: StrongDesc<Sampler>,
	pub screen_size_recip: Vec2,
	pub flags: ParamFlags,
}

#[derive(Clone, Copy, BufferStruct)]
pub struct ParamFlags(u32);

bitflags! {
	impl ParamFlags: u32 {
		const SRGB_FRAMEBUFFER = 1;
		const FONT_TEXTURE = 2;
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
) {
	let vertex = param.vertices.access(&descriptors).load(vertex_id as usize);
	let screen_pos = vertex.pos;
	*position = Vec4::new(
		2.0 * screen_pos.x * param.screen_size_recip.x - 1.0,
		2.0 * screen_pos.y * param.screen_size_recip.y - 1.0,
		1.,
		1.,
	);
	*vtx_color_srgb = vertex.color();
	*vtx_uv = vertex.uv;
}

#[bindless(fragment())]
pub fn egui_fragment(
	#[bindless(descriptors)] descriptors: Descriptors,
	#[bindless(param)] param: &Param<'static>,
	vtx_color_srgb: Vec4,
	vtx_uv: Vec2,
	frag_color: &mut Vec4,
) {
	let image = unsafe { param.image.to_transient_unchecked(&descriptors) };

	// our image is unorm, but the values stored inside are in srgb colorspace
	// this sample op should not do any srgb conversions
	let mut image_srgb: Vec4 = image
		.access(&descriptors)
		.sample(param.sampler.access(&descriptors), vtx_uv);
	if param.flags.contains(ParamFlags::FONT_TEXTURE) {
		image_srgb = image_srgb.xxxx();
	}

	let out_color_srgb = vtx_color_srgb * image_srgb;
	*frag_color = if param.flags.contains(ParamFlags::SRGB_FRAMEBUFFER) {
		out_color_srgb
	} else {
		linear_to_srgb_alpha(out_color_srgb)
	}
}
