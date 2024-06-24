mod gpu {
	use crate::meshlet::instance::MeshletInstance;
	use crate::meshlet::mesh::MeshletMesh;
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::reference::DescStructRef;
	use vulkano_bindless_shaders::descriptor::{Buffer, Desc, DescRef};

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	pub struct MeshletMesh2Instance<R: DescRef, RR: DescStructRef + 'static> {
		pub mesh: Desc<R, Buffer<MeshletMesh<RR>>>,
		pub instances: Desc<R, Buffer<[MeshletInstance]>>,
	}
}

pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::meshlet::instance::MeshletInstance;
	use crate::meshlet::mesh::MeshletMeshDisk;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Archive, Serialize, Deserialize)]
	pub struct MeshletMesh2InstanceDisk {
		pub mesh: MeshletMeshDisk,
		pub instances: Vec<MeshletInstance>,
	}
}

#[cfg(feature = "disk")]
pub use disk::*;
