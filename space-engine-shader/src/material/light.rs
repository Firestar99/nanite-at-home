use crate::material::radiance::Radiance;
use glam::{Vec3, Vec3A};

#[derive(Copy, Clone)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
pub struct DirectionalLight {
	pub direction: Vec3,
	pub color: Radiance,
}
#[derive(Copy, Clone, bytemuck_derive::AnyBitPattern)]
pub struct DirectionalLightTransfer {
	direction: Vec3A,
	color: <Radiance as vulkano_bindless_shaders::buffer_content::BufferStruct>::Transfer,
}

unsafe impl vulkano_bindless_shaders::buffer_content::BufferStruct for DirectionalLight
where
	DirectionalLight: Copy,
{
	type Transfer = DirectionalLightTransfer;
	unsafe fn write_cpu(
		self,
		meta: &mut impl vulkano_bindless_shaders::buffer_content::MetadataCpuInterface,
	) -> Self::Transfer {
		Self::Transfer {
			direction: Vec3A::from(self.direction),
			color: vulkano_bindless_shaders::buffer_content::BufferStruct::write_cpu(self.color, meta),
		}
	}
	unsafe fn read(from: Self::Transfer, meta: vulkano_bindless_shaders::descriptor::metadata::Metadata) -> Self {
		Self {
			direction: Vec3::from(from.direction),
			color: vulkano_bindless_shaders::buffer_content::BufferStruct::read(from.color, meta),
		}
	}
}

#[derive(Copy, Clone)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
pub struct PointLight {
	pub position: Vec3,
	pub color: Radiance,
}

#[derive(Copy, Clone, bytemuck_derive::AnyBitPattern)]
pub struct PointLightTransfer {
	position: Vec3A,
	color: <Radiance as vulkano_bindless_shaders::buffer_content::BufferStruct>::Transfer,
}
unsafe impl vulkano_bindless_shaders::buffer_content::BufferStruct for PointLight
where
	PointLight: Copy,
{
	type Transfer = PointLightTransfer;
	unsafe fn write_cpu(
		self,
		meta: &mut impl vulkano_bindless_shaders::buffer_content::MetadataCpuInterface,
	) -> Self::Transfer {
		Self::Transfer {
			position: Vec3A::from(self.position),
			color: vulkano_bindless_shaders::buffer_content::BufferStruct::write_cpu(self.color, meta),
		}
	}
	unsafe fn read(from: Self::Transfer, meta: vulkano_bindless_shaders::descriptor::metadata::Metadata) -> Self {
		Self {
			position: Vec3::from(from.position),
			color: vulkano_bindless_shaders::buffer_content::BufferStruct::read(from.color, meta),
		}
	}
}
