pub mod buffer;
pub mod descriptor_type;
pub mod descriptors;
pub mod images;
pub mod reference;
pub mod sampler;

pub use buffer::*;
pub use descriptor_type::*;
pub use descriptors::*;
pub use images::*;
pub use reference::*;
pub use sampler::*;

pub const BINDING_BUFFER: u32 = 0;
pub const BINDING_STORAGE_IMAGE: u32 = 1;
pub const BINDING_SAMPLED_IMAGE: u32 = 2;
pub const BINDING_SAMPLER: u32 = 3;
