mod decode;
mod disk;
mod metadata;
mod runtime;
mod size;

pub use metadata::*;
pub use runtime::*;
pub use size::*;
use std::fmt::{Debug, Display, Formatter};

use glam::UVec3;
use rkyv::{Archive, Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[repr(C)]
#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct Image2DDisk<const IMAGE_TYPE: u32> {
	pub bytes: Arc<[u8]>,
	pub metadata: Image2DMetadata<{ IMAGE_TYPE }>,
}
