use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::mutable::{MutDesc, MutDescExt};
use crate::descriptor::{Bindless, BindlessCreateInfo, Image};
use crate::platform::BindlessPlatform;
use ash::vk::ImageUsageFlags;
use rust_gpu_bindless_shaders::descriptor::SampleType;
use rust_gpu_bindless_shaders::spirv_std::image::Image2d;
use std::ops::Deref;
use std::sync::Arc;

impl<
		SampledType: SampleType<FORMAT, COMPONENTS> + Send + Sync + 'static,
		const DIM: u32,
		const DEPTH: u32,
		const ARRAYED: u32,
		const MULTISAMPLED: u32,
		const SAMPLED: u32,
		const FORMAT: u32,
		const COMPONENTS: u32,
	> DescContentCpu for Image<SampledType, DIM, DEPTH, ARRAYED, MULTISAMPLED, SAMPLED, FORMAT, COMPONENTS>
{
	type DescTable<P: BindlessPlatform> = ImageTable<P>;
	type VulkanType<P: BindlessPlatform> = ImageSlot<P>;

	fn deref_table<P: BindlessPlatform>(slot: &RcTableSlot) -> &Self::VulkanType<P> {
		slot.try_deref::<ImageInterface<P>>().unwrap()
	}
}

impl<P: BindlessPlatform> DescTable for ImageTable<P> {}

pub struct ImageSlot<P: BindlessPlatform> {
	pub image: P::Image,
	pub imageview: P::ImageView,
	pub memory_allocation: P::MemoryAllocation,
	pub usage: ImageUsageFlags,
}

pub struct ImageTable<P: BindlessPlatform> {
	table: Arc<Table<ImageInterface<P>>>,
}

impl<P: BindlessPlatform> ImageTable<P> {
	pub fn new(
		table_sync: &Arc<TableSync>,
		ci: Arc<BindlessCreateInfo<P>>,
		global_descriptor_set: P::BindlessDescriptorSet,
	) -> Self {
		let counts = ci.counts.image;
		let interface = ImageInterface {
			ci,
			global_descriptor_set,
		};
		Self {
			table: table_sync.register(counts, interface).unwrap(),
		}
	}
}

pub struct ImageTableAccess<'a, P: BindlessPlatform>(pub &'a Arc<Bindless<P>>);

impl<'a, P: BindlessPlatform> Deref for ImageTableAccess<'a, P> {
	type Target = ImageTable<P>;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.image
	}
}

impl<'a, P: BindlessPlatform> ImageTableAccess<'a, P> {
	/// Allocates a new slot for this image and imageview
	///
	/// # Safety
	/// Image's device and ImageView's device must be the same as the bindless device. Ownership of the Image and
	/// ImageView is transferred to this table. You may not access or drop it afterward, except by going though the
	/// returned `RCDesc`.
	#[inline]
	pub unsafe fn alloc_slot_2d(&self, image: ImageSlot<P>) -> MutDesc<P, Image2d> {
		unsafe {
			MutDesc::new(
				self.table
					.alloc_slot(image)
					.map_err(|a| format!("ImageTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub(crate) fn flush_queue(&self) -> DrainFlushQueue<'_, ImageInterface<P>> {
		self.table.drain_flush_queue()
	}
}

pub struct ImageInterface<P: BindlessPlatform> {
	ci: Arc<BindlessCreateInfo<P>>,
	global_descriptor_set: P::BindlessDescriptorSet,
}

impl<P: BindlessPlatform> TableInterface for ImageInterface<P> {
	type Slot = ImageSlot<P>;

	fn drop_slots<'a>(&self, indices: impl DescriptorIndexIterator<'a, Self>) {
		unsafe {
			P::destroy_images(&self.ci, &self.global_descriptor_set, indices);
		}
	}

	fn flush<'a>(&self, _flush_queue: impl DescriptorIndexIterator<'a, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
