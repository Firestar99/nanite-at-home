use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::rc_slots::RCSlot;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorBindingFlags, DescriptorSetLayoutBinding};
use vulkano::device::physical::PhysicalDevice;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::{DescType, ResourceTable};

/// A descriptor type to some resource, that may have generic arguments to specify its contents.
pub trait DescTypeCpu: DescType {
	/// Associated non-generic [`ResourceTableCpu`]
	type ResourceTableCpu: ResourceTableCpu;

	/// CPU type exposed externally, that may contain extra generic type information
	type CpuType;

	/// deref [`Self::TableType`] to exposed [`Self::CpuType`]
	fn deref_table(slot: &RCSlot<<Self::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> &Self::CpuType;

	/// turn [`Self::CpuType`] into the internal [`Self::ResourceTableCpu::SlotType`]
	#[allow(clippy::wrong_self_convention)]
	fn to_table(from: Self::CpuType) -> <Self::ResourceTableCpu as ResourceTableCpu>::SlotType;
}

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor binding.
pub trait ResourceTableCpu: ResourceTable {
	/// internal non-generic type used within the resource table
	type SlotType: Clone;

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
