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
// mod pending;

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
	pub type Bindless = crate::generic::descriptor::Bindless<crate::P>;
	pub type BindlessFrame = crate::generic::descriptor::BindlessFrame<crate::P>;
	pub type StrongMetadataCpu<'a> = crate::generic::descriptor::StrongMetadataCpu<'a, crate::P>;
	pub type BackingRefsError = crate::generic::descriptor::BackingRefsError;
	pub type BufferTable = crate::generic::descriptor::BufferTable<crate::P>;
	pub type BufferSlot = crate::generic::descriptor::BufferSlot<crate::P>;
	pub type BufferTableAccess<'a> = crate::generic::descriptor::BufferTableAccess<'a, crate::P>;
	pub type BufferAllocationError = crate::generic::descriptor::BufferAllocationError<crate::P>;
	pub type BufferInterface = crate::generic::descriptor::BufferInterface<crate::P>;
	pub type StrongBackingRefs = crate::generic::descriptor::StrongBackingRefs<crate::P>;
	pub type ImageTable = crate::generic::descriptor::ImageTable<crate::P>;
	pub type ImageSlot = crate::generic::descriptor::ImageSlot<crate::P>;
	pub type ImageTableAccess<'a> = crate::generic::descriptor::ImageTableAccess<'a, crate::P>;
	pub type ImageAllocationError = crate::generic::descriptor::ImageAllocationError<crate::P>;
	pub type ImageInterface = crate::generic::descriptor::ImageInterface<crate::P>;
	pub type Mut = crate::generic::descriptor::Mut<crate::P>;
	pub type MutDesc<C> = crate::generic::descriptor::MutDesc<crate::P, C>;
	pub type RC = crate::generic::descriptor::RC<crate::P>;
	pub type RCDesc<C> = crate::generic::descriptor::RCDesc<crate::P, C>;
	pub type AnyRCDesc = crate::generic::descriptor::AnyRCDesc<crate::P>;
	pub type SamplerTable = crate::generic::descriptor::SamplerTable<crate::P>;
	pub type SamplerTableAccess<'a> = crate::generic::descriptor::SamplerTableAccess<'a, crate::P>;
	pub type SamplerInterface = crate::generic::descriptor::SamplerInterface<crate::P>;

	pub use crate::generic::descriptor::*;
}
