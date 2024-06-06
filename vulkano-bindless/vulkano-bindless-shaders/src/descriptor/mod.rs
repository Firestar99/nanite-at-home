pub mod buffer;
pub mod descriptor_content;
pub mod descriptors;
pub mod image;
pub mod reference;
pub mod sampler;

pub mod metadata;

#[path = "../../../image_types.rs"]
#[macro_use]
mod image_types;

pub use buffer::{Buffer, BufferSlice};
pub use descriptor_content::DescContent;
pub use descriptors::Descriptors;
pub use image::Image;
pub use reference::{TransientDesc, ValidDesc, WeakDesc};
pub use sampler::Sampler;

pub const BINDING_BUFFER: u32 = 0;
pub const BINDING_STORAGE_IMAGE: u32 = 1;
pub const BINDING_SAMPLED_IMAGE: u32 = 2;
pub const BINDING_SAMPLER: u32 = 3;
