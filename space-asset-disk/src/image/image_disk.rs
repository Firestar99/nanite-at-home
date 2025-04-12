use crate::image::{DiskImageMetadata, Image, ImageType};
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// An Image that can be stored on disk
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct ImageDisk<const IMAGE_TYPE: u32> {
	pub meta: DiskImageMetadata<IMAGE_TYPE>,
	pub data: Arc<[u8]>,
}

impl<const IMAGE_TYPE: u32> Image<'_, DiskImageMetadata<IMAGE_TYPE>> {
	pub fn into_image_disk(self) -> ImageDisk<IMAGE_TYPE> {
		ImageDisk {
			meta: self.meta,
			data: Arc::from(self.data),
		}
	}
}

impl<const IMAGE_TYPE: u32> ImageDisk<IMAGE_TYPE> {
	pub fn to_image(&self) -> Image<'_, DiskImageMetadata<IMAGE_TYPE>> {
		Image {
			meta: self.meta,
			data: Cow::Borrowed(&self.data),
		}
	}
}

impl<const IMAGE_TYPE: u32> ArchivedImageDisk<IMAGE_TYPE> {
	pub fn to_image(&self) -> Image<'_, DiskImageMetadata<IMAGE_TYPE>> {
		Image {
			meta: self.meta.deserialize(&mut rkyv::Infallible).unwrap(),
			data: Cow::Borrowed(&self.data),
		}
	}
}

pub type ImageDiskRLinear = ImageDisk<{ ImageType::R_VALUE as u32 }>;
pub type ImageDiskRgLinear = ImageDisk<{ ImageType::RG_VALUE as u32 }>;
pub type ImageDiskRgbaLinear = ImageDisk<{ ImageType::RGBA_LINEAR as u32 }>;
pub type ImageDiskRgbaSrgb = ImageDisk<{ ImageType::RGBA_COLOR as u32 }>;
