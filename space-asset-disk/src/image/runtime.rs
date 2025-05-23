use crate::image::ImageType;
use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::ops::Range;

/// An Image that can be read directly by the GPU, like [`BCnImage`] and [`UncompressedImage`].
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct RuntimeImage<'a> {
	pub meta: RuntimeImageMetadata,
	pub data: Cow<'a, [u8]>,
}

impl RuntimeImage<'_> {
	pub fn to_static(self) -> RuntimeImage<'static> {
		RuntimeImage {
			meta: self.meta,
			data: Cow::Owned(self.data.into_owned()),
		}
	}
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub enum RuntimeImageCompression {
	None,
	BCn,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct RuntimeImageMetadata {
	pub image_type: ImageType,
	pub compression: RuntimeImageCompression,
	pub extent: UVec3,
	pub block_size: UVec3,
	pub bytes_per_block: u32,
	pub mip_levels: u32,
	/// The size in bytes of the entire image with all mips.
	pub total_size: usize,
}

impl RuntimeImageMetadata {
	/// The amount of mips in a complete mip chain for an image sized `extent`.
	///
	/// See https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#resources-image-mip-level-sizing
	pub fn complete_mip_chain_levels(extent: UVec3) -> u32 {
		u32::ilog2(u32::max(u32::max(extent.x, extent.y), extent.z)) + 1
	}

	/// Calculates the image extent of a particular `mip` for an image sized `extent`.
	///
	/// See https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#resources-image-mip-level-sizing
	pub fn extent_for_mip_level(extent: UVec3, mip: u32) -> UVec3 {
		UVec3::max(extent / (1 << mip), UVec3::ONE)
	}

	/// Calculates the image extent of a particular `mip`.
	pub fn mip_extent(&self, mip: u32) -> UVec3 {
		Self::extent_for_mip_level(self.extent, mip)
	}

	/// Query the size in bytes of a single mip layer.
	pub fn mip_size(&self, mip: u32) -> usize {
		let mip_extent = self.mip_extent(mip);
		mip_extent.x.div_ceil(self.block_size.x) as usize
			* mip_extent.y.div_ceil(self.block_size.y) as usize
			* mip_extent.z.div_ceil(self.block_size.z) as usize
			* self.bytes_per_block as usize
	}

	pub fn mip_range(&self, mip: u32) -> Range<usize> {
		let start = self.mip_start(mip);
		start..start + self.mip_size(mip)
	}

	/// Query the start offset of a mip layer.
	pub fn mip_start(&self, mip: u32) -> usize {
		(0..mip).map(|i| self.mip_size(i)).sum()
	}

	pub fn new(
		image_type: ImageType,
		compression: RuntimeImageCompression,
		extent: UVec3,
		block_size: UVec3,
		bytes_per_block: u32,
		mip_levels: u32,
	) -> Self {
		let mut out = Self {
			image_type,
			compression,
			extent,
			block_size,
			bytes_per_block,
			mip_levels,
			total_size: 0,
		};
		out.total_size = out.mip_start(mip_levels);
		out
	}

	pub fn new_uncompressed(
		image_type: ImageType,
		extent: UVec3,
		bytes_per_pixel: u32,
		mip_levels: u32,
	) -> RuntimeImageMetadata {
		Self::new(
			image_type,
			RuntimeImageCompression::None,
			extent,
			UVec3::ONE,
			bytes_per_pixel,
			mip_levels,
		)
	}
}
