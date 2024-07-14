use bytemuck_derive::{Pod, Zeroable};
use core::mem;
use glam::{Vec2, Vec3};
use static_assertions::const_assert_eq;

#[cfg(not(target_arch = "spirv"))]
use core::fmt::{Debug, DebugStruct, Formatter};

#[repr(C)]
#[derive(Copy, Clone, Default)]
#[cfg_attr(not(target_arch = "spirv"), derive(Zeroable, Pod))]
pub struct PbrVertex {
	pub normals: Vec3,
	pub tex_coords: Vec2,
}

impl PbrVertex {
	pub fn encode(&self) -> EncodedPbrVertex {
		EncodedPbrVertex {
			normals: self.normals.to_array(),
			tex_coords: self.tex_coords.to_array(),
		}
	}

	#[cfg(not(target_arch = "spirv"))]
	fn debug_struct(&self, mut debug: DebugStruct) -> core::fmt::Result {
		debug
			.field("normals", &self.normals)
			.field("tex_coords", &self.tex_coords)
			.finish()
	}
}

#[cfg(not(target_arch = "spirv"))]
impl Debug for PbrVertex {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		self.debug_struct(f.debug_struct("MaterialVertex"))
	}
}

#[repr(C)]
#[derive(Copy, Clone, Default, Zeroable, Pod)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct EncodedPbrVertex {
	normals: [f32; 3],
	tex_coords: [f32; 2],
}
const_assert_eq!(mem::size_of::<EncodedPbrVertex>(), 5 * 4);

impl EncodedPbrVertex {
	pub fn decode(&self) -> PbrVertex {
		PbrVertex {
			normals: Vec3::from(self.normals),
			tex_coords: Vec2::from(self.tex_coords),
		}
	}
}

#[cfg(not(target_arch = "spirv"))]
impl Debug for EncodedPbrVertex {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		self.decode().debug_struct(f.debug_struct("EncodedMaterialVertex"))
	}
}
