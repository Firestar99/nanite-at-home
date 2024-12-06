use crate::backing::table::RcTableSlot;
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::DescContent;

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescContentCpu: DescContent {
	// FIXME should I move some of these types and functions to the table, to not have to duplicate them between shared and mut DescContent?
	/// Associated non-generic [`DescTable`]
	type DescTable<P: BindlessPlatform>: DescTable;

	/// CPU type exposed externally, that may contain extra generic type information
	type VulkanType<P: BindlessPlatform>;

	/// CPU type exposed externally, that may contain extra generic type information
	type Slot<P: BindlessPlatform>;

	/// deref [`Self::TableType`] to exposed [`Self::VulkanType`]
	fn get_slot<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::Slot<P>;

	/// deref [`Self::TableType`] to exposed [`Self::VulkanType`]
	fn deref_table<P: BindlessPlatform>(slot: &Self::Slot<P>) -> &Self::VulkanType<P>;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait DescTable: Sized {}

pub trait DescContentMutCpu: DescContentCpu {
	type Shared: DescContentCpu;
}
