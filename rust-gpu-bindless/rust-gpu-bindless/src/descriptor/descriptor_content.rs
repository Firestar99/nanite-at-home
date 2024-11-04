use crate::backend::table::RcTableSlot;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::DescContent;

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescContentCpu: DescContent {
	/// Associated non-generic [`DescTable`]
	type DescTable<P: BindlessPlatform>: DescTable;

	/// CPU type exposed externally, that may contain extra generic type information
	type VulkanType<P: BindlessPlatform>;

	/// deref [`Self::TableType`] to exposed [`Self::VulkanType`]
	fn deref_table<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::VulkanType<P>;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait DescTable: Sized {}
