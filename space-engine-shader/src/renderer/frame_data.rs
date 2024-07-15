use crate::renderer::camera::Camera;
use bytemuck_derive::AnyBitPattern;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
pub enum DebugSettings {
	None,
	MeshletIdOverlay,
	MeshletId,
	VertexNormals,
	VertexTexCoords,
}

impl DebugSettings {
	pub const MAX_VALUE: DebugSettings = DebugSettings::VertexTexCoords;
	pub const LEN: u32 = Self::MAX_VALUE as u32 + 1;
}

#[derive(Copy, Clone, AnyBitPattern)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
	pub debug_settings: u32,
}

impl FrameData {
	pub fn debug_settings(&self) -> DebugSettings {
		DebugSettings::try_from(self.debug_settings).unwrap_or(DebugSettings::None)
	}
}
