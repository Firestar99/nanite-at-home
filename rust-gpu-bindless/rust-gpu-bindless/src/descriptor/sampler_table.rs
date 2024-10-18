use crate::backend::range_set::{range_to_descriptor_index, DescriptorIndexRangeSet};
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::{Bindless, RCDescExt};
use rust_gpu_bindless_shaders::descriptor::Sampler;
use rust_gpu_bindless_shaders::descriptor::BINDING_SAMPLER;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, InvalidateDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::image::sampler::{Sampler as VSampler, SamplerCreateInfo};
use vulkano::shader::ShaderStages;
use vulkano::{Validated, VulkanError};

impl DescContentCpu for Sampler {
	type DescTable = SamplerTable;
	type VulkanType = Arc<VSampler>;

	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType {
		slot.try_deref::<SamplerInterface>().unwrap()
	}
}

impl DescTable for SamplerTable {
	type Slot = Arc<VSampler>;

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

pub struct SamplerTable {
	table: Arc<Table<SamplerInterface>>,
}

impl SamplerTable {
	pub fn new(table_sync: &Arc<TableSync>, descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			table: table_sync.register(count, SamplerInterface { descriptor_set }).unwrap(),
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
		unsafe {
			RCDesc::new(
				self.table
					.alloc_slot(sampler)
					.map_err(|a| format!("SamplerTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub fn alloc(&self, sampler_create_info: SamplerCreateInfo) -> Result<RCDesc<Sampler>, Validated<VulkanError>> {
		let sampler = VSampler::new(self.0.device.clone(), sampler_create_info)?;
		Ok(self.alloc_slot(sampler))
	}

	pub(crate) fn flush_descriptors(
		&self,
		delay_drop: &mut Vec<RcTableSlot>,
		mut writes: impl FnMut(WriteDescriptorSet),
	) {
		let flush_queue = self.table.drain_flush_queue();
		let mut set = DescriptorIndexRangeSet::new();
		delay_drop.reserve(flush_queue.size_hint().0);
		for x in flush_queue {
			set.insert(x.id().index());
			delay_drop.push(x);
		}
		for range in set.iter_ranges() {
			writes(WriteDescriptorSet::sampler_array(
				BINDING_SAMPLER,
				range.start.to_u32(),
				range_to_descriptor_index(range).map(|index| unsafe { self.table.get_slot_unchecked(index).clone() }),
			));
		}
	}
}

pub struct SamplerInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl TableInterface for SamplerInterface {
	type Slot = Arc<VSampler>;

	fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
		for x in indices.iter_ranges() {
			self.descriptor_set
				.invalidate(&[InvalidateDescriptorSet::invalidate_array(
					BINDING_SAMPLER,
					x.start.to_u32(),
					(x.end - x.start) as u32,
				)])
				.unwrap();
		}
	}

	fn flush(&self, _flush_queue: DrainFlushQueue<'_, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
