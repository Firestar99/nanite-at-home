use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::{Bindless, DescriptorBinding, RCDescExt, VulkanDescriptorType};
use crate::platform::interface::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::Sampler;
use rust_gpu_bindless_shaders::descriptor::BINDING_SAMPLER;
use std::ops::Deref;
use std::sync::Arc;

impl<P: BindlessPlatform> DescContentCpu for Sampler {
	type DescTable = SamplerTable<P>;
	type VulkanType = P::Sampler;

	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType {
		slot.try_deref::<SamplerInterface<P>>().unwrap()
	}
}

impl<P: BindlessPlatform> DescTable for SamplerTable<P> {
	fn layout_binding(count: DescriptorCounts) -> impl Iterator<Item = DescriptorBinding> {
		[DescriptorBinding {
			ty: VulkanDescriptorType::Sampler,
			binding: BINDING_SAMPLER,
			count: count.samplers,
		}]
		.into_iter()
	}
}

pub struct SamplerTable<P: BindlessPlatform> {
	table: Arc<Table<SamplerInterface<P>>>,
}

impl<P: BindlessPlatform> SamplerTable<P> {
	pub fn new(
		table_sync: &Arc<TableSync>,
		device: P::Device,
		global_descriptor_set: P::DescriptorSet,
		count: u32,
	) -> Self {
		Self {
			table: table_sync
				.register(
					count,
					SamplerInterface {
						device,
						global_descriptor_set,
					},
				)
				.unwrap(),
		}
	}
}

pub struct SamplerTableAccess<'a, P: BindlessPlatform>(pub &'a Arc<Bindless<P>>);

impl<'a, P: BindlessPlatform> Deref for SamplerTableAccess<'a, P> {
	type Target = SamplerTable<P>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.sampler
	}
}

impl<'a, P: BindlessPlatform> SamplerTableAccess<'a, P> {
	/// Allocates a new slot for this sampler
	///
	/// # Safety
	/// Sampler's device must be the same as the bindless device. Ownership of the sampler is transferred to this table.
	/// You may not access or drop it afterward, except by going though the returned `RCDesc`.
	#[inline]
	pub unsafe fn alloc_slot(&self, sampler: P::Sampler) -> RCDesc<Sampler> {
		unsafe {
			RCDesc::new(
				self.table
					.alloc_slot(sampler)
					.map_err(|a| format!("SamplerTable: {}", a))
					.unwrap(),
			)
		}
	}

	// pub(crate) fn flush_descriptors(
	// 	&self,
	// 	delay_drop: &mut Vec<RcTableSlot>,
	// 	mut writes: impl FnMut(WriteDescriptorSet),
	// ) {
	// 	let flush_queue = self.table.drain_flush_queue();
	// 	let mut set = DescriptorIndexRangeSet::from();
	// 	delay_drop.reserve(flush_queue.size_hint().0);
	// 	for x in flush_queue {
	// 		set.insert(x.id().index());
	// 		delay_drop.push(x);
	// 	}
	// 	for range in set.iter_ranges() {
	// 		writes(WriteDescriptorSet::sampler_array(
	// 			BINDING_SAMPLER,
	// 			range.start.to_u32(),
	// 			range_to_descriptor_index(range).map(|index| unsafe { self.table.get_slot_unchecked(index).clone() }),
	// 		));
	// 	}
	// }
}

pub struct SamplerInterface<P: BindlessPlatform> {
	device: P::Device,
	global_descriptor_set: P::DescriptorSet,
}

impl<P: BindlessPlatform> TableInterface for SamplerInterface<P> {
	type Slot = P::Sampler;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			P::destroy_samplers(
				&self.device,
				&self.global_descriptor_set,
				indices.into_iter().map(|(_, s)| s),
			);
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
