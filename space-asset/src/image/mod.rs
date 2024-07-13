#![cfg(feature = "disk")]

mod decode;
mod metadata;
mod size;
mod upload;

pub use metadata::*;
pub use size::*;

use rkyv::{Archive, Deserialize, Serialize};

#[repr(C)]
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct Image2DDisk<const DATA_TYPE: u32> {
	pub bytes: Box<[u8]>,
	pub metadata: Image2DMetadata<{ DATA_TYPE }>,
}
