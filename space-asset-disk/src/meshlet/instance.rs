use glam::Affine3A;
use vulkano_bindless_macros::BufferContentPlain;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, BufferContentPlain, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct MeshletInstanceDisk {
	pub transform: Affine3A,
}

impl MeshletInstanceDisk {
	pub fn new(transform: Affine3A) -> Self {
		Self { transform }
	}
}
