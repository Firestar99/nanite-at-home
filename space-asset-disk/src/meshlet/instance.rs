use glam::Affine3A;
use vulkano_bindless_macros::BufferContent;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContent, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct MeshletInstanceDisk {
	pub transform: Affine3A,
}

impl MeshletInstanceDisk {
	pub fn new(transform: Affine3A) -> Self {
		Self { transform }
	}
}
