use crate::descriptor::descriptor_content::{DescContentCpu, DescTable, DescTableEnum, DescTableEnumType};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::{FlushUpdates, ResourceTable, TableEpochGuard};
use crate::descriptor::Bindless;
use crate::rc_slot::{RCSlotsInterface, SlotIndex};
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
use vulkano_bindless_shaders::descriptor::DescContentType;
use vulkano_bindless_shaders::descriptor::Sampler;
use vulkano_bindless_shaders::descriptor::BINDING_SAMPLER;

impl DescContentCpu for Sampler {
	type DescTable = SamplerTable;
	type VulkanType = Arc<VSampler>;

	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType {
		slot
	}
}

impl DescTable for SamplerTable {
	const CONTENT_ENUM: DescContentType = DescContentType::Sampler;
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

	#[inline]
	fn lock_table(&self) -> TableEpochGuard<Self> {
		self.resource_table.epoch_guard()
	}

	#[inline]
	fn table_enum_new<A: DescTableEnumType>(inner: A::Type<Self>) -> DescTableEnum<A> {
		DescTableEnum::Sampler(inner)
	}

	#[inline]
	fn table_enum_try_deref<A: DescTableEnumType>(table_enum: &DescTableEnum<A>) -> Option<&A::Type<Self>> {
		if let DescTableEnum::Sampler(v) = table_enum {
			Some(v)
		} else {
			None
		}
	}

	#[inline]
	fn table_enum_try_into<A: DescTableEnumType>(
		table_enum: DescTableEnum<A>,
	) -> Result<A::Type<Self>, DescTableEnum<A>> {
		if let DescTableEnum::Sampler(v) = table_enum {
			Ok(v)
		} else {
			Err(table_enum)
		}
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

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.sampler
	}
}

impl<'a> SamplerTableAccess<'a> {
	#[inline]
	pub fn alloc_slot(&self, sampler: Arc<VSampler>) -> RCDesc<Sampler> {
		self.resource_table
			.alloc_slot(sampler)
			.map_err(|a| format!("SamplerTable: {}", a))
			.unwrap()
	}

	pub fn alloc(&self, sampler_create_info: SamplerCreateInfo) -> Result<RCDesc<Sampler>, Validated<VulkanError>> {
		let sampler = VSampler::new(self.device.clone(), sampler_create_info)?;
		Ok(self.alloc_slot(sampler))
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
