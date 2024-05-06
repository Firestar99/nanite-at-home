use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::ResourceTable;
use crate::rc_slots::RCSlot;
use std::sync::Arc;
use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::image::sampler::{Sampler as VSampler, SamplerCreateInfo};
use vulkano::{Validated, VulkanError};
use vulkano_bindless_shaders::descriptor::sampler::{Sampler, SamplerTable};

impl DescTypeCpu for Sampler {
	type ResourceTableCpu = SamplerTable;
	type CpuType = Arc<VSampler>;

	fn deref_table(slot: &RCSlot<<Self::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> &Self::CpuType {
		slot
	}

	fn to_table(from: Self::CpuType) -> <Self::ResourceTableCpu as ResourceTableCpu>::SlotType {
		from
	}
}

impl ResourceTableCpu for SamplerTable {
	type SlotType = Arc<VSampler>;
	const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::Sampler;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_samplers
			.unwrap()
	}

	fn write_descriptor_set(
		binding: u32,
		first_array_element: u32,
		elements: impl IntoIterator<Item = Self::SlotType>,
	) -> WriteDescriptorSet {
		WriteDescriptorSet::sampler_array(binding, first_array_element, elements)
	}
}

pub struct SamplerResourceTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<SamplerTable>,
}

impl SamplerResourceTable {
	pub fn new(device: Arc<Device>, count: u32) -> Self {
		Self {
			device,
			resource_table: ResourceTable::new(count),
		}
	}

	#[inline]
	pub fn alloc_slot(&self, sampler: Arc<VSampler>) -> RCDesc<Sampler> {
		self.resource_table.alloc_slot(sampler)
	}

	pub fn alloc(&self, sampler_create_info: SamplerCreateInfo) -> Result<RCDesc<Sampler>, Validated<VulkanError>> {
		let sampler = VSampler::new(self.device.clone(), sampler_create_info)?;
		Ok(self.resource_table.alloc_slot(sampler))
	}
}
