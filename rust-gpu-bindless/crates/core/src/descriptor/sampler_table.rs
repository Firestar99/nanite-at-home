use crate::backing::range_set::DescriptorIndexIterator;
use crate::backing::table::{DrainFlushQueue, RcTableSlot, SlotAllocationError, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::rc::RCDesc;
use crate::descriptor::{Bindless, DescriptorCounts, RCDescExt, WeakBindless};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::Sampler;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;

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
	pub fn new(table_sync: &Arc<TableSync>, counts: DescriptorCounts, bindless: WeakBindless<P>) -> Self {
		Self {
			table: table_sync
				.register(counts.samplers, SamplerInterface { bindless })
				.unwrap(),
		}
	}
}

pub struct SamplerTableAccess<'a, P: BindlessPlatform>(pub &'a Bindless<P>);

impl<P: BindlessPlatform> Deref for SamplerTableAccess<'_, P> {
	type Target = SamplerTable<P>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.sampler
	}
}

/// Texel mixing mode when sampling between texels.
///
/// Docs copied from wgpu.
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub enum Filter {
	/// Nearest neighbor sampling.
	///
	/// This creates a pixelated effect when used as a mag filter
	#[default]
	Nearest = 0,
	/// Linear Interpolation
	///
	/// This makes textures smooth but blurry when used as a mag filter.
	Linear = 1,
}

/// How edges should be handled in texture addressing.
///
/// Docs copied from wgpu.
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Hash)]
pub enum AddressMode {
	/// Clamp the value to the edge of the texture
	///
	/// -0.25 -> 0.0
	/// 1.25  -> 1.0
	#[default]
	ClampToEdge = 0,
	/// Repeat the texture in a tiling fashion
	///
	/// -0.25 -> 0.75
	/// 1.25 -> 0.25
	Repeat = 1,
	/// Repeat the texture, mirroring it every repeat
	///
	/// -0.25 -> 0.25
	/// 1.25 -> 0.75
	MirrorRepeat = 2,
	/// Clamp the value to the border of the texture
	/// Requires feature [`Features::ADDRESS_MODE_CLAMP_TO_BORDER`]
	///
	/// -0.25 -> border
	/// 1.25 -> border
	ClampToBorder = 3,
}

/// Color variation to use when sampler addressing mode is [`AddressMode::ClampToBorder`]
///
/// Docs copied from wgpu.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum BorderColor {
	/// [0, 0, 0, 0]
	#[default]
	TransparentBlack,
	/// [0, 0, 0, 1]
	OpaqueBlack,
	/// [1, 1, 1, 1]
	OpaqueWhite,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct BindlessSamplerCreateInfo {
	pub mag_filter: Filter,
	pub min_filter: Filter,
	pub mipmap_mode: Filter,
	pub address_mode_u: AddressMode,
	pub address_mode_v: AddressMode,
	pub address_mode_w: AddressMode,
	pub max_anisotropy: Option<f32>,
	pub min_lod: f32,
	pub max_lod: Option<f32>,
	pub border_color: BorderColor,
}

#[derive(Error)]
pub enum SamplerAllocationError<P: BindlessPlatform> {
	#[error("Platform Error: {0}")]
	Platform(#[source] P::AllocationError),
	#[error("Slot Allocation Error: {0}")]
	Slot(#[from] SlotAllocationError),
}

impl<P: BindlessPlatform> Debug for SamplerAllocationError<P> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self, f)
	}
}

impl<P: BindlessPlatform> SamplerTableAccess<'_, P> {
	/// Allocates a new slot for this sampler
	///
	/// # Safety
	/// Sampler's device must be the same as the bindless device. Ownership of the sampler is transferred to this table.
	/// You may not access or drop it afterward, except by going though the returned `RCDesc`.
	#[inline]
	pub unsafe fn alloc_slot(&self, sampler: P::Sampler) -> Result<RCDesc<P, Sampler>, SlotAllocationError> {
		unsafe { Ok(RCDesc::new(self.table.alloc_slot(sampler)?)) }
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, SamplerInterface<P>> {
		self.table.drain_flush_queue()
	}

	pub fn alloc(
		&self,
		create_info: &BindlessSamplerCreateInfo,
	) -> Result<RCDesc<P, Sampler>, SamplerAllocationError<P>> {
		unsafe {
			let sampler = self
				.0
				.platform
				.alloc_sampler(create_info)
				.map_err(Into::<SamplerAllocationError<P>>::into)?;
			Ok(self.alloc_slot(sampler)?)
		}
	}
}

pub struct SamplerInterface<P: BindlessPlatform> {
	bindless: WeakBindless<P>,
}

impl<P: BindlessPlatform> TableInterface for SamplerInterface<P> {
	type Slot = P::Sampler;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			if let Some(bindless) = self.bindless.upgrade() {
				bindless
					.platform
					.destroy_samplers(bindless.global_descriptor_set(), indices);
			}
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
