pub mod bindless;
pub mod buffer_metadata_cpu;
pub mod buffer_table;
pub mod descriptor_content;
pub mod descriptor_counts;
pub mod image_table;
pub mod mutable;
pub mod rc;
pub mod sampler_table;

pub use bindless::*;
pub use buffer_table::*;
pub use descriptor_content::*;
pub use descriptor_counts::*;
pub use image_table::*;
pub use rc::*;
pub use rust_gpu_bindless_shaders::descriptor::*;
pub use sampler_table::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BindlessAllocationScheme {
	/// Perform a dedicated, driver-managed allocation for the given buffer or image, allowing it to perform
	/// optimizations on this type of allocation.
	Dedicated,
	/// The memory for this resource will be allocated and managed by gpu-allocator.
	AllocatorManaged,
}
