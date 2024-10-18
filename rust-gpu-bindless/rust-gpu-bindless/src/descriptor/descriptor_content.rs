use crate::backend::table::RcTableSlot;
use crate::descriptor::descriptor_counts::DescriptorCounts;
use rust_gpu_bindless_shaders::descriptor::DescContent;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayoutBinding};
use vulkano::device::physical::PhysicalDevice;
use vulkano::shader::ShaderStages;

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
	/// internal non-generic type used within the resource table
	type Slot;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32;

	const BINDING_FLAGS: DescriptorBindingFlags = DescriptorBindingFlags::UPDATE_AFTER_BIND
		.union(DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING)
		.union(DescriptorBindingFlags::PARTIALLY_BOUND);

	fn layout_binding(
		stages: ShaderStages,
		count: DescriptorCounts,
		out: &mut BTreeMap<u32, DescriptorSetLayoutBinding>,
	);
}
