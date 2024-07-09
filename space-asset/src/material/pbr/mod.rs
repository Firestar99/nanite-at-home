pub mod vertex;

mod gpu {
	use crate::material::pbr::vertex::{EncodedPbrVertex, PbrVertex};
	use crate::meshlet::vertex::MaterialVertexId;
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::{AliveDescRef, Buffer, Desc, DescRef, Descriptors};
	use vulkano_bindless_shaders::spirv_std::image::Image2d;

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	pub struct PbrMaterial<R: DescRef> {
		pub vertices: Desc<R, Buffer<[EncodedPbrVertex]>>,
		pub base_color: Desc<R, Image2d>,
		pub base_color_factor: [f32; 4],
		pub normal: Desc<R, Image2d>,
		pub normal_scale: f32,
		pub omr: Desc<R, Image2d>,
		pub occlusion_strength: f32,
		pub metallic_factor: f32,
		pub roughness_factor: f32,
	}

	impl<R: AliveDescRef> PbrMaterial<R> {
		pub fn load_vertex(&self, descriptors: &Descriptors, index: MaterialVertexId) -> PbrVertex {
			self.vertices.access(descriptors).load(index.0 as usize).decode()
		}

		/// # Safety
		/// index must be in bounds
		pub unsafe fn load_vertex_unchecked(&self, descriptors: &Descriptors, index: MaterialVertexId) -> PbrVertex {
			unsafe {
				self.vertices
					.access(descriptors)
					.load_unchecked(index.0 as usize)
					.decode()
			}
		}
	}
}
pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::image::Image2DDisk;
	use crate::material::pbr::vertex::EncodedPbrVertex;
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct PbrMaterialDisk {
		pub vertices: Vec<EncodedPbrVertex>,
		pub base_color: Option<Image2DDisk>,
		pub base_color_factor: [f32; 4],
		pub normal: Option<Image2DDisk>,
		pub normal_scale: f32,
		pub omr: Option<Image2DDisk>,
		pub occlusion_strength: f32,
		pub metallic_factor: f32,
		pub roughness_factor: f32,
	}
}
#[cfg(feature = "disk")]
pub use disk::*;

#[cfg(feature = "runtime")]
mod runtime {
	use crate::material::pbr::{ArchivedPbrMaterialDisk, PbrMaterial};
	use crate::uploader::{deserialize_infallible, UploadError, Uploader};
	use vulkano::Validated;
	use vulkano_bindless::descriptor::{RCDescExt, RC};
	use vulkano_bindless_shaders::descriptor::reference::Strong;

	impl PbrMaterial<RC> {
		pub fn to_strong(&self) -> PbrMaterial<Strong> {
			PbrMaterial {
				vertices: self.vertices.to_strong(),
				base_color: self.base_color.to_strong(),
				base_color_factor: self.base_color_factor,
				normal: self.normal.to_strong(),
				normal_scale: self.normal_scale,
				omr: self.omr.to_strong(),
				occlusion_strength: self.occlusion_strength,
				metallic_factor: self.metallic_factor,
				roughness_factor: self.roughness_factor,
			}
		}
	}

	impl ArchivedPbrMaterialDisk {
		pub async fn upload(&self, uploader: &Uploader) -> Result<PbrMaterial<RC>, Validated<UploadError>> {
			let vertices = uploader.upload_buffer_iter(self.vertices.iter().map(deserialize_infallible));
			let base_color = self.base_color.as_ref().unwrap().upload(uploader);
			let normal = self.normal.as_ref().unwrap().upload(uploader);
			let omr = self.omr.as_ref().unwrap().upload(uploader);
			Ok(PbrMaterial {
				vertices: vertices.await?.into(),
				base_color: base_color.await?,
				base_color_factor: self.base_color_factor,
				normal: normal.await?,
				normal_scale: self.normal_scale,
				omr: omr.await?,
				occlusion_strength: self.occlusion_strength,
				metallic_factor: self.metallic_factor,
				roughness_factor: self.roughness_factor,
			})
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
