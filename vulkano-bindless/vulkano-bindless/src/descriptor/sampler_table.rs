use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::ResourceTable;
use crate::rc_slots::RCSlot;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::image::sampler::{Sampler as VSampler, SamplerCreateInfo};
use vulkano::shader::ShaderStages;
use vulkano::{Validated, VulkanError};
use vulkano_bindless_shaders::descriptor::sampler::{Sampler, SamplerTable};
use vulkano_bindless_shaders::descriptor::BINDING_SAMPLER;

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

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_samplers
			.unwrap()
	}

	fn layout_binding(
		stages: ShaderStages,
		count: DescriptorCounts,
		out: &mut BTreeMap<u32, DescriptorSetLayoutBinding>,
	) {
		out.insert(
			BINDING_SAMPLER,
			DescriptorSetLayoutBinding {
				binding_flags: Self::BINDING_FLAGS,
				descriptor_count: count.samplers,
				stages,
				..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::Sampler)
			},
		)
		.ok_or(())
		.unwrap_err();
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

	pub(crate) fn flush_updates<const C: usize>(&self, writes: &mut SmallVec<[WriteDescriptorSet; C]>) {
		self.resource_table.flush_updates(|first_array_element, buffer| {
			writes.push(WriteDescriptorSet::sampler_array(
				BINDING_SAMPLER,
				first_array_element,
				buffer.drain(..),
			));
		})
	}
}
