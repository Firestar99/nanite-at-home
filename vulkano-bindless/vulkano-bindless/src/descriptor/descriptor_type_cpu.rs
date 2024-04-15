use crate::rc_slots::RCSlot;
use std::sync::Arc;
use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
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

/// In a resource table descriptors of varying generic arguments can be stored and are sent to the GPU in a single descriptor set of a single kind.
pub trait ResourceTableCpu: ResourceTable {
	/// Type used within the [`RCSlot`]
	type SlotType: Clone;

	const DESCRIPTOR_TYPE: DescriptorType;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32;

	fn write_descriptor_set(
		binding: u32,
		first_array_element: u32,
		elements: impl IntoIterator<Item = Self::SlotType>,
	) -> WriteDescriptorSet;
}
