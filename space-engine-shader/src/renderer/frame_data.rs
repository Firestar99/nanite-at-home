use crate::material::light::DirectionalLight;
use crate::material::radiance::Radiance;
use crate::renderer::camera::Camera;
use crate::renderer::lod_selection::LodSelection;
use glam::UVec2;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rust_gpu_bindless_macros::BufferStruct;
use space_asset_shader::shape::sphere::ProjectToScreen;

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
pub enum DebugSettings {
	None,
	MeshletIdOverlay,
	TriangleIdOverlay,
	MeshletId,
	TriangleId,
	BaseColor,
	Normals,
	VertexNormals,
	RoughnessMetallic,
	ReconstructedPosition,
}

impl DebugSettings {
	pub const MAX_VALUE: DebugSettings = DebugSettings::ReconstructedPosition;
	pub const LEN: u32 = Self::MAX_VALUE as u32 + 1;
}

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
	pub viewport_size: UVec2,
	pub project_to_screen: ProjectToScreen,
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
