use glam::{Vec2, Vec4, vec4};
use rust_gpu_bindless_macros::BufferStructPlain;

/// Vertex represents an egui vertex and should match it exactly
#[repr(C)]
#[derive(Copy, Clone, Debug, BufferStructPlain)]
pub struct Vertex {
	/// Logical pixel coordinates (points).
	/// (0,0) is the top left corner of the screen.
	pub pos: Vec2, // 64 bit

	/// Normalized texture coordinates.
	/// (0, 0) is the top left corner of the texture.
	/// (1, 1) is the bottom right corner of the texture.
	pub uv: Vec2, // 64 bit

	/// sRGBA with premultiplied alpha
	pub color: u32, // 32 bit
}

impl Vertex {
	pub fn color(&self) -> Vec4 {
		unpack_color(self.color)
	}
}

// SRGB u32 -> Vec4 in 0. .. 1.
pub fn unpack_color(color: u32) -> Vec4 {
	vec4(
		(color & 255) as f32,
		((color >> 8) & 255) as f32,
		((color >> 16) & 255) as f32,
		((color >> 24) & 255) as f32,
	) / 255.0
}

// [u8; 4] SRGB -> u32
pub fn pack_color(color: [u8; 4]) -> u32 {
	(color[0] as u32) | (color[1] as u32) << 8 | (color[2] as u32) << 16 | (color[3] as u32) << 24
}

#[cfg(feature = "epaint")]
mod epaint {
	use super::*;
	use ::epaint::Vertex as EVertex;
	use core::mem::size_of;
	use glam::vec2;
	use static_assertions::const_assert_eq;

	const_assert_eq!(size_of::<Vertex>(), size_of::<EVertex>());

	impl From<EVertex> for Vertex {
		fn from(value: EVertex) -> Self {
			Vertex {
				pos: vec2(value.pos.x, value.pos.y),
				uv: vec2(value.uv.x, value.uv.y),
				color: pack_color(value.color.to_array()),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::vertex::{pack_color, unpack_color};

	#[test]
	fn test_color_packing() {
		for pattern in [[0; 4], [255; 4], [1, 2, 3, 4], [0x81; 4]] {
			for value in 0..255u8 {
				for i in 0..4 {
					let mut color = pattern;
					color[i] = value;

					let converted = unpack_color(pack_color(color)).to_array().map(|f| (f * 255.) as u8);
					assert_eq!(color, converted)
				}
			}
		}
	}
}
