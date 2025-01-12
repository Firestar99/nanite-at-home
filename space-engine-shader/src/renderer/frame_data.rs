use crate::material::light::DirectionalLight;
use crate::material::radiance::Radiance;
use crate::renderer::camera::Camera;
use crate::renderer::lod_selection::LodSelection;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rust_gpu_bindless_macros::BufferStruct;

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
pub enum DebugSettings {
	None,
	LodLevelOverlay,
	TriangleIdOverlay,
	MeshletIdOverlay,
	LodLevel,
	TriangleId,
	MeshletId,
	BaseColor,
	Normals,
	VertexNormals,
	RoughnessMetallic,
}

impl DebugSettings {
	pub const MAX_VALUE: DebugSettings = DebugSettings::RoughnessMetallic;
	pub const LEN: u32 = Self::MAX_VALUE as u32 + 1;
}

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
	pub nanite_error_threshold: f32,
	pub debug_settings: u32,
	pub debug_lod_level: LodSelection,
	pub sun: DirectionalLight,
	pub ambient_light: Radiance,
}

impl FrameData {
	pub fn debug_settings(&self) -> DebugSettings {
		DebugSettings::try_from(self.debug_settings).unwrap_or(DebugSettings::None)
	}
}
