pub mod vertex;

mod gpu {
	use vulkano_bindless_macros::BufferContent;
	use vulkano_bindless_shaders::descriptor::{Desc, DescRef};
	use vulkano_bindless_shaders::spirv_std::image::Image2d;

	#[repr(C)]
	#[derive(Copy, Clone, BufferContent)]
	pub struct PbrMaterial<R: DescRef> {
		pub base_color: Desc<R, Image2d>,
		pub base_color_factor: [f32; 4],
		pub normal: Desc<R, Image2d>,
		pub normal_scale: f32,
		pub omr: Desc<R, Image2d>,
		pub occlusion_strength: f32,
		pub metallic_factor: f32,
		pub roughness_factor: f32,
	}
}
pub use gpu::*;

#[cfg(feature = "disk")]
mod disk {
	use crate::image::{Image2DDisk, ImageType};
	use rkyv::{Archive, Deserialize, Serialize};

	#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
	pub struct PbrMaterialDisk {
		pub base_color: Option<Image2DDisk<{ ImageType::RGBA_COLOR as u32 }>>,
		pub base_color_factor: [f32; 4],
		pub normal: Option<Image2DDisk<{ ImageType::RG_VALUES as u32 }>>,
		pub normal_scale: f32,
		pub omr: Option<Image2DDisk<{ ImageType::RGBA_LINEAR as u32 }>>,
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
	use crate::uploader::{UploadError, Uploader};
	use std::future::Future;
	use vulkano::Validated;
	use vulkano_bindless::descriptor::{RCDescExt, RC};
	use vulkano_bindless_shaders::descriptor::reference::Strong;

	impl PbrMaterial<RC> {
		pub fn to_strong(&self) -> PbrMaterial<Strong> {
			PbrMaterial {
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
		pub fn upload<'a>(
			&'a self,
			uploader: &'a Uploader,
		) -> impl Future<Output = Result<PbrMaterial<RC>, Validated<UploadError>>> + 'a {
			let base_color = self.base_color.as_ref().map(|tex| tex.upload(uploader));
			let normal = self.normal.as_ref().map(|tex| tex.upload(uploader));
			let omr = self.omr.as_ref().map(|tex| tex.upload(uploader));
			async {
				Ok(PbrMaterial {
					base_color: uploader.await_or_white_texture(base_color).await?,
					base_color_factor: self.base_color_factor,
					normal: uploader.await_or_white_texture(normal).await?,
					normal_scale: self.normal_scale,
					omr: uploader.await_or_white_texture(omr).await?,
					occlusion_strength: self.occlusion_strength,
					metallic_factor: self.metallic_factor,
					roughness_factor: self.roughness_factor,
				})
			}
		}
	}
}
#[cfg(feature = "runtime")]
pub use runtime::*;
