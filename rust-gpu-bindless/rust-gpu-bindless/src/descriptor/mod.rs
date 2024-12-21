mod bindless;
mod buffer_metadata_cpu;
mod buffer_table;
mod descriptor_content;
mod descriptor_counts;
mod extent;
mod image_table;
mod mutdesc;
mod rc;
mod sampler_table;

pub use bindless::*;
pub use buffer_metadata_cpu::*;
pub use buffer_table::*;
pub use descriptor_content::*;
pub use descriptor_counts::*;
pub use extent::*;
pub use image_table::*;
pub use mutdesc::*;
pub use rc::*;
pub use rust_gpu_bindless_shaders::descriptor::*;
pub use sampler_table::*;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum BindlessAllocationScheme {
	/// Perform a dedicated, driver-managed allocation for the given buffer or image, allowing it to perform
	/// optimizations on this type of allocation.
	Dedicated,
	/// The memory for this resource will be allocated and managed by gpu-allocator.
	#[default]
	AllocatorManaged,
}
