mod buffer;
mod descriptor_content;
mod descriptors;
mod image;
mod metadata;
mod reference;
mod sampler;

#[path = "../../../image_types.rs"]
#[macro_use]
mod image_types;

pub use buffer::*;
pub use descriptor_content::{DescContent, DescContentEnum};
pub use descriptors::*;
pub use image::*;
pub use metadata::*;
pub use reference::*;
pub use sampler::*;

pub const BINDING_BUFFER: u32 = 0;
pub const BINDING_STORAGE_IMAGE: u32 = 1;
pub const BINDING_SAMPLED_IMAGE: u32 = 2;
pub const BINDING_SAMPLER: u32 = 3;
