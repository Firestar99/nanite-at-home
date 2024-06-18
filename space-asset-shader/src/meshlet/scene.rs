use crate::meshlet::mesh::MeshletMesh;
use glam::{Affine3A, Mat3A, Vec3A};
use vulkano_bindless_macros::DescStruct;
use vulkano_bindless_shaders::descriptor::{Buffer, Desc, DescRef};

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletInstance<R: DescRef + 'static> {
	pub transform: [f32; 12],
	pub mesh: Desc<R, Buffer<MeshletMesh<R>>>,
}

impl<R: DescRef + 'static> MeshletInstance<R> {
	pub fn new(mesh: Desc<R, Buffer<MeshletMesh<R>>>, transform: Affine3A) -> Self {
		Self {
			transform: transform.to_cols_array(),
			mesh,
		}
	}

	pub fn transform(&self) -> Affine3A {
		affine3a_from_cols_array(self.transform)
	}
}

/// same as `Affine3A::from_cols_array(&transform)` but doesn't use slices
pub const fn affine3a_from_cols_array(transform: [f32; 12]) -> Affine3A {
	let mat = [
		transform[0],
		transform[1],
		transform[2],
		transform[3],
		transform[4],
		transform[5],
		transform[6],
		transform[7],
		transform[8],
	];
	let vec = [transform[9], transform[10], transform[11]];
	Affine3A {
		matrix3: Mat3A::from_cols_array(&mat),
		translation: Vec3A::from_array(vec),
	}
}

#[derive(Copy, Clone, DescStruct)]
#[repr(C)]
pub struct MeshletScene<R: DescRef + 'static> {
	pub instances: Desc<R, Buffer<[MeshletInstance<R>]>>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn transform_from() {
		let affine3a = Affine3A::from_cols_array(&[0., 1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.]);
		assert_eq!(affine3a, affine3a_from_cols_array(affine3a.to_cols_array()));
	}
}
