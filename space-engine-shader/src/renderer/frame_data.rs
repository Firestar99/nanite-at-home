use crate::material::light::DirectionalLight;
use crate::material::radiance::Radiance;
use crate::renderer::camera::Camera;
use crate::renderer::lod_selection::LodSelection;
use num_enum::{FromPrimitive, IntoPrimitive};
use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::buffer_content::BufferStructPlain;

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, FromPrimitive, IntoPrimitive)]
pub enum DebugSettings {
	#[default]
	None,
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

unsafe impl BufferStructPlain for DebugSettings {
	type Transfer = u32;

	unsafe fn write(self) -> Self::Transfer {
		<u32 as From<Self>>::from(self)
	}

	unsafe fn read(from: Self::Transfer) -> Self {
		<Self as num_enum::FromPrimitive>::from_primitive(from)
	}
}

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct NaniteSettings {
	pub error_threshold: f32,
	pub bounding_sphere_scale: f32,
}

#[derive(Copy, Clone, BufferStruct)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[repr(C)]
pub struct FrameData {
	pub camera: Camera,
	pub debug_settings: DebugSettings,
	pub debug_mix: f32,
	pub debug_lod_level: LodSelection,
	pub sun: DirectionalLight,
	pub ambient_light: Radiance,
	pub nanite: NaniteSettings,
}
