use core::mem;
use glam::{Affine3A, Mat3, Vec3};
use static_assertions::const_assert_eq;

/// Affine transformation like [`Affine3A`] but also stores a matrix to transform normals.
#[repr(C)]
#[derive(Copy, Clone, Default)]
#[cfg_attr(not(target_arch = "spirv"), derive(Debug))]
#[cfg_attr(feature = "disk", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct AffineTransform {
	pub affine: Affine3A,
	pub normals: Mat3,
}

impl AffineTransform {
	pub fn new(transform: Affine3A) -> Self {
		Self {
			affine: transform,
			normals: Mat3::from(transform.matrix3).inverse().transpose(),
		}
	}

	pub fn translation(&self) -> Vec3 {
		Vec3::from(self.affine.translation)
	}
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck_derive::AnyBitPattern)]
pub struct AffineTransformTransfer {
	transform: [f32; 12],
	transform_normals: [f32; 9],
	_pad: [f32; 3],
}
const_assert_eq!(mem::size_of::<AffineTransformTransfer>(), 24 * 4);

unsafe impl vulkano_bindless_shaders::buffer_content::BufferStruct for AffineTransform
where
	AffineTransform: Copy,
{
	type Transfer = AffineTransformTransfer;

	unsafe fn write_cpu(
		self,
		_meta: &mut impl vulkano_bindless_shaders::buffer_content::MetadataCpuInterface,
	) -> Self::Transfer {
		Self::Transfer {
			transform: self.affine.to_cols_array(),
			transform_normals: self.normals.to_cols_array(),
			_pad: [0.; 3],
		}
	}

	unsafe fn read(from: Self::Transfer, _meta: vulkano_bindless_shaders::descriptor::metadata::Metadata) -> Self {
		Self {
			affine: Affine3A::from_cols_array(&from.transform),
			normals: Mat3::from_cols_array(&from.transform_normals),
		}
	}
}
