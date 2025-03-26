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

#[cfg(feature = "primary")]
pub(crate) mod primary {
	pub type Bindless = bindless::Bindless<crate::P>;
	pub type BindlessFrame = bindless::BindlessFrame<crate::P>;
	pub type StrongMetadataCpu<'a> = buffer_metadata_cpu::StrongMetadataCpu<'a, crate::P>;
	pub type BackingRefsError = buffer_metadata_cpu::BackingRefsError;
	pub type BufferTable = buffer_table::BufferTable<crate::P>;
	pub type BufferSlot = buffer_table::BufferSlot<crate::P>;
	pub type BufferTableAccess<'a> = buffer_table::BufferTableAccess<'a, crate::P>;
	pub type BufferAllocationError = buffer_table::BufferAllocationError<crate::P>;
	pub type BufferInterface = buffer_table::BufferInterface<crate::P>;
	pub type StrongBackingRefs = buffer_table::StrongBackingRefs<crate::P>;
	pub type ImageTable = image_table::ImageTable<crate::P>;
	pub type ImageSlot = image_table::ImageSlot<crate::P>;
	pub type ImageTableAccess<'a> = image_table::ImageTableAccess<'a, crate::P>;
	pub type ImageAllocationError = image_table::ImageAllocationError<crate::P>;
	pub type ImageInterface = image_table::ImageInterface<crate::P>;
	pub type Mut = mutdesc::Mut<crate::P>;
	pub type MutDesc<C> = mutdesc::MutDesc<crate::P, C>;
	pub type RC = rc::RC<crate::P>;
	pub type RCDesc<C> = rc::RCDesc<crate::P, C>;
	pub type AnyRCDesc = rc::AnyRCDesc<crate::P>;
	pub type SamplerTable = sampler_table::SamplerTable<crate::P>;
	pub type SamplerTableAccess<'a> = sampler_table::SamplerTableAccess<'a, crate::P>;
	pub type SamplerInterface = sampler_table::SamplerInterface<crate::P>;

	pub use crate::generic::descriptor::*;
}
