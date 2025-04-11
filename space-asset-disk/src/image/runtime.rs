use crate::image::ImageType;
use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;

/// An Image that can be read directly by the GPU, like [`BCnImage`] and [`UncompressedImage`].
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct RuntimeImage<'a> {
	pub meta: RuntimeImageMetadata,
	pub data: Cow<'a, [u8]>,
}

impl<'a> RuntimeImage<'a> {
	pub fn to_static(self) -> RuntimeImage<'static> {
		RuntimeImage {
			meta: self.meta,
			data: Cow::Owned(self.data.into_owned()),
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Archive, Serialize, Deserialize)]
pub struct RuntimeImageMetadata {
	image_type: ImageType,
	block_count: UVec3,
	block_size: UVec3,
	bytes_per_block: u32,
	mip_layers: u32,
}

impl RuntimeImageMetadata {
	pub fn new(
		image_type: ImageType,
		block_count: UVec3,
		block_size: UVec3,
		bytes_per_block: u32,
		mip_layers: u32,
	) -> Result<Self, RuntimeImageMetadataError> {
		let mask = (1 << mip_layers) - 1;
		if block_count.x & mask != 0 || block_count.y & mask != 0 || block_count.z & mask != 0 {
			Err(RuntimeImageMetadataError::InvalidMipLayers {
				block_count,
				mip_layers,
			})
		} else {
			Ok(Self {
				image_type,
				block_count,
				bytes_per_block,
				block_size,
				mip_layers,
			})
		}
	}

	pub fn new_uncompressed(
		image_type: ImageType,
		extent: UVec3,
		bytes_per_pixel: u32,
		mip_layers: u32,
	) -> Result<Self, RuntimeImageMetadataError> {
		Self::new(image_type, extent, UVec3::ONE, bytes_per_pixel, mip_layers)
	}

	pub fn block_count(&self) -> UVec3 {
		self.block_count
	}

	pub fn bytes_per_block(&self) -> u32 {
		self.bytes_per_block
	}

	pub fn block_size(&self) -> UVec3 {
		self.block_size
	}

	pub fn mip_layers(&self) -> u32 {
		self.mip_layers
	}

	/// The Image extent (of mip 0)
	pub fn extent(&self) -> UVec3 {
		self.block_size * self.block_count
	}

	/// Query the size in bytes of a single mip layer. Returns `None` if the mip is out of bounds.
	pub fn size_of_mip(&self, mip: u32) -> Option<usize> {
		if mip < self.mip_layers {
			let blocks = (self.block_size.x >> mip) * (self.block_size.y >> mip) * (self.block_size.z >> mip);
			Some((blocks * self.bytes_per_block) as usize)
		} else {
			None
		}
	}

	/// Query the size in bytes of the entire image with all mips.
	pub fn size_of_total(&self) -> usize {
		let mut out = 0;
		for i in 0..self.mip_layers {
			out += self.size_of_mip(i).unwrap();
		}
		out
	}
}

#[derive(Error)]
pub enum RuntimeImageMetadataError {
	#[error("block_count {} must be cleanly divisible by {} to have {} clean mip layers", block_count, 1 << mip_layers, mip_layers)]
	InvalidMipLayers { block_count: UVec3, mip_layers: u32 },
}

impl Debug for RuntimeImageMetadataError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}
