use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::rc::RCDesc;
use crate::descriptor::{Bindless, DescriptorCounts, RCDescExt};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::Sampler;
use std::ops::Deref;
use std::sync::{Arc, Weak};

impl DescContentCpu for Sampler {
	type DescTable<P: BindlessPlatform> = SamplerTable<P>;
}

impl<P: BindlessPlatform> DescTable<P> for SamplerTable<P> {
	type Slot = P::Sampler;

	fn get_slot(slot: &RcTableSlot) -> &Self::Slot {
		slot.try_deref::<SamplerInterface<P>>().unwrap()
	}
}

pub struct SamplerTable<P: BindlessPlatform> {
	table: Arc<Table<SamplerInterface<P>>>,
}

impl<P: BindlessPlatform> SamplerTable<P> {
	pub fn new(table_sync: &Arc<TableSync>, counts: DescriptorCounts, bindless: Weak<Bindless<P>>) -> Self {
		Self {
			table: table_sync
				.register(counts.samplers, SamplerInterface { bindless })
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
	pub unsafe fn alloc_slot(&self, sampler: P::Sampler) -> RCDesc<P, Sampler> {
		unsafe {
			RCDesc::new(
				self.table
					.alloc_slot(sampler)
					.map_err(|a| format!("SamplerTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, SamplerInterface<P>> {
		self.table.drain_flush_queue()
	}
}

pub struct SamplerInterface<P: BindlessPlatform> {
	bindless: Weak<Bindless<P>>,
}

impl<P: BindlessPlatform> TableInterface for SamplerInterface<P> {
	type Slot = P::Sampler;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			if let Some(bindless) = self.bindless.upgrade() {
				bindless
					.platform
					.destroy_samplers(&bindless.global_descriptor_set(), indices);
			}
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
