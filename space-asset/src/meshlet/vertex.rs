use bytemuck_derive::{Pod, Zeroable};
use core::mem;
use glam::Vec3;
use static_assertions::const_assert_eq;

#[cfg(not(target_arch = "spirv"))]
use core::fmt::{Debug, DebugStruct, Formatter};

#[repr(C)]
#[derive(Copy, Clone, Default, Zeroable, Pod)]
pub struct DrawVertex {
	pub position: Vec3,
	// vertex_id: u32, // in the future?
}

impl DrawVertex {
	pub fn encode(&self) -> EncodedDrawVertex {
		EncodedDrawVertex {
			position: self.position.to_array(),
		}
	}

	#[cfg(not(target_arch = "spirv"))]
	fn debug_struct(&self, mut debug: DebugStruct) -> core::fmt::Result {
		debug.field("position", &self.position).finish()
	}
}

#[cfg(not(target_arch = "spirv"))]
impl Debug for DrawVertex {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		self.debug_struct(f.debug_struct("DrawVertex"))
	}
}

#[repr(C)]
#[derive(Copy, Clone, Default, Zeroable, Pod)]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct EncodedDrawVertex {
	pub position: [f32; 3],
	// vertex_id: u32, // in the future?
}
const_assert_eq!(mem::size_of::<EncodedDrawVertex>(), 3 * 4);

impl EncodedDrawVertex {
	pub fn decode(&self) -> DrawVertex {
		DrawVertex {
			position: Vec3::from(self.position),
		}
	}
}

#[cfg(not(target_arch = "spirv"))]
impl Debug for EncodedDrawVertex {
	fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
		self.decode().debug_struct(f.debug_struct("EncodedDrawVertex"))
	}
}
