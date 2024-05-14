pub mod buffer;
pub mod descriptor_type;
pub mod descriptors;
pub mod images;
pub mod reference;
pub mod sampler;

pub mod metadata;

#[path = "../../../image_types.rs"]
#[macro_use]
mod image_types;

pub use buffer::{Buffer, BufferSlice};
pub use descriptor_type::DescType;
pub use descriptors::Descriptors;
pub use images::Image;
pub use reference::{TransientDesc, ValidDesc, WeakDesc};
pub use sampler::Sampler;

pub const BINDING_BUFFER: u32 = 0;
pub const BINDING_STORAGE_IMAGE: u32 = 1;
pub const BINDING_SAMPLED_IMAGE: u32 = 2;
pub const BINDING_SAMPLER: u32 = 3;
