use crate::meshlet::instance::MeshletInstance;
use crate::meshlet::mesh::MeshletMesh;
use vulkano_bindless_macros::BufferContent;
use vulkano_bindless_shaders::descriptor::{Buffer, Desc, DescRef, DescStructRef};

#[repr(C)]
#[derive(Copy, Clone, BufferContent)]
pub struct MeshletMesh2Instance<R: DescRef, RR: DescStructRef + 'static> {
	pub mesh: Desc<R, Buffer<MeshletMesh<RR>>>,
	pub instances: Desc<R, Buffer<[MeshletInstance]>>,
}
