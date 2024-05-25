use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::{DescTable, DescTypeCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::{FlushUpdates, Lock, ResourceTable};
use crate::descriptor::Bindless;
use crate::rc_slots::{RCSlotsInterface, SlotIndex};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, InvalidateDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Device, DeviceOwned};
use vulkano::image::sampler::{Sampler as VSampler, SamplerCreateInfo};
use vulkano::shader::ShaderStages;
use vulkano::{Validated, VulkanError};
use vulkano_bindless_shaders::descriptor::descriptor_type::DescEnum;
use vulkano_bindless_shaders::descriptor::sampler::Sampler;
use vulkano_bindless_shaders::descriptor::BINDING_SAMPLER;

impl DescTypeCpu for Sampler {
	type DescTable = SamplerTable;
	type VulkanType = Arc<VSampler>;

	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType {
		slot
	}
}

impl DescTable for SamplerTable {
	const DESC_ENUM: DescEnum = DescEnum::Sampler;
	type Slot = Arc<VSampler>;
	type RCSlotsInterface = SamplerInterface;

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

	fn lock_table(&self) -> Lock<Self> {
		self.resource_table.lock()
	}
}

pub struct SamplerTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<SamplerTable>,
}

impl SamplerTable {
	pub fn new(descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			device: descriptor_set.device().clone(),
			resource_table: ResourceTable::new(count, SamplerInterface { descriptor_set }),
		}
	}
}

pub struct SamplerTableAccess<'a>(pub &'a Arc<Bindless>);

impl<'a> Deref for SamplerTableAccess<'a> {
	type Target = SamplerTable;

	fn deref(&self) -> &Self::Target {
		&self.0.sampler
	}
}

impl<'a> SamplerTableAccess<'a> {
	#[inline]
	pub fn alloc_slot(&self, sampler: Arc<VSampler>) -> RCDesc<Sampler> {
		self.resource_table.alloc_slot(sampler)
	}

	pub fn alloc(&self, sampler_create_info: SamplerCreateInfo) -> Result<RCDesc<Sampler>, Validated<VulkanError>> {
		let sampler = VSampler::new(self.device.clone(), sampler_create_info)?;
		Ok(self.resource_table.alloc_slot(sampler))
	}
}

impl SamplerTable {
	pub(crate) fn flush_updates(&self, mut writes: impl FnMut(WriteDescriptorSet)) -> FlushUpdates<SamplerTable> {
		let flush_updates = self.resource_table.flush_updates();
		flush_updates.iter(|first_array_element, buffer| {
			writes(WriteDescriptorSet::sampler_array(
				BINDING_SAMPLER,
				first_array_element,
				buffer.iter().cloned(),
			));
		});
		flush_updates
	}
}

pub struct SamplerInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl RCSlotsInterface<<SamplerTable as DescTable>::Slot> for SamplerInterface {
	fn drop_slot(&self, index: SlotIndex, t: <SamplerTable as DescTable>::Slot) {
		self.descriptor_set
			.invalidate(&[InvalidateDescriptorSet::invalidate_array(
				BINDING_SAMPLER,
				index.0 as u32,
				1,
			)])
			.unwrap();
		drop(t);
	}
}
