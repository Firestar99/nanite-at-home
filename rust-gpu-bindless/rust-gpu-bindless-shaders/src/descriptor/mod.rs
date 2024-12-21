mod buffer;
mod descriptor_content;
mod descriptors;
mod id;
mod image;
mod predefined_image;
mod reference;
mod sampler;
mod strong;
mod transient;
mod weak;

#[path = "../../../image_types.rs"]
#[macro_use]
mod image_types;

pub use buffer::*;
pub use descriptor_content::*;
pub use descriptors::*;
pub use id::*;
pub use image::*;
pub use predefined_image::*;
pub use reference::*;
pub use sampler::*;
pub use strong::*;
pub use transient::*;
pub use weak::*;

pub const BINDING_BUFFER: u32 = 0;
pub const BINDING_STORAGE_IMAGE: u32 = 1;
pub const BINDING_SAMPLED_IMAGE: u32 = 2;
pub const BINDING_SAMPLER: u32 = 3;
