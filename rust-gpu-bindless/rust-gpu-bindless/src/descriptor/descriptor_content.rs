use crate::backend::table::RcTableSlot;
use crate::descriptor::descriptor_counts::DescriptorCounts;
use rust_gpu_bindless_shaders::descriptor::DescContent;

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescContentCpu: DescContent {
	/// Associated non-generic [`DescTable`]
	type DescTable: DescTable;

	/// CPU type exposed externally, that may contain extra generic type information
	type VulkanType;

	/// deref [`Self::TableType`] to exposed [`Self::VulkanType`]
	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait DescTable: Sized {
	fn layout_binding(count: DescriptorCounts) -> impl Iterator<Item = DescriptorBinding>;
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum VulkanDescriptorType {
	Buffer,
	SampledImage,
	StorageImage,
	Sampler,
}

#[derive(Copy, Clone, Debug)]
pub struct DescriptorBinding {
	pub ty: VulkanDescriptorType,
	pub binding: u32,
	pub count: u32,
}
