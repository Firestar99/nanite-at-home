use crate::backend::range_set::DescriptorIndexIterator;
use crate::backend::table::{RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::mutable::{MutDesc, MutDescExt};
use crate::descriptor::{Bindless, BindlessCreateInfo, DescriptorBinding, Image, VulkanDescriptorType};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::descriptor::SampleType;
use rust_gpu_bindless_shaders::descriptor::{BINDING_SAMPLED_IMAGE, BINDING_STORAGE_IMAGE};
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

impl<P: BindlessPlatform> DescTable for ImageTable<P> {
	fn layout_binding(count: DescriptorCounts) -> impl Iterator<Item = DescriptorBinding> {
		[
			DescriptorBinding {
				ty: VulkanDescriptorType::StorageImage,
				binding: BINDING_STORAGE_IMAGE,
				count: count.image,
			},
			DescriptorBinding {
				ty: VulkanDescriptorType::SampledImage,
				binding: BINDING_SAMPLED_IMAGE,
				count: count.image,
			},
		]
		.into_iter()
	}
}

pub struct ImageSlot<P: BindlessPlatform> {
	pub image: P::Image,
	pub imageview: P::ImageView,
	pub memory_allocation: P::MemoryAllocation,
}

pub struct ImageTable<P: BindlessPlatform> {
	table: Arc<Table<ImageInterface<P>>>,
}

impl<P: BindlessPlatform> ImageTable<P> {
	pub fn new(
		table_sync: &Arc<TableSync>,
		ci: Arc<BindlessCreateInfo<P>>,
		global_descriptor_set: P::DescriptorSet,
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

	// pub(crate) fn flush_descriptors(
	// 	&self,
	// 	delay_drop: &mut Vec<RcTableSlot>,
	// 	mut writes: impl FnMut(WriteDescriptorSet),
	// ) {
	// 	let flush_queue = self.table.drain_flush_queue();
	// 	let mut storage = DescriptorIndexRangeSet::from();
	// 	let mut sampled = DescriptorIndexRangeSet::from();
	// 	delay_drop.reserve(flush_queue.size_hint().0);
	// 	for x in flush_queue {
	// 		let image = unsafe { self.table.get_slot_unchecked(x.id().index()) };
	// 		if image.usage().contains(ImageUsage::STORAGE) {
	// 			storage.insert(x.id().index());
	// 		}
	// 		if image.usage().contains(ImageUsage::SAMPLED) {
	// 			sampled.insert(x.id().index());
	// 		}
	// 		delay_drop.push(x);
	// 	}
	//
	// 	for (binding, range_set) in [(BINDING_STORAGE_IMAGE, storage), (BINDING_SAMPLED_IMAGE, sampled)] {
	// 		for range in range_set.iter_ranges() {
	// 			writes(WriteDescriptorSet::image_view_array(
	// 				binding,
	// 				range.start.to_u32(),
	// 				range_to_descriptor_index(range)
	// 					.map(|index| unsafe { self.table.get_slot_unchecked(index).clone() }),
	// 			));
	// 		}
	// 	}
	// }
}

pub struct ImageInterface<P: BindlessPlatform> {
	ci: Arc<BindlessCreateInfo<P>>,
	global_descriptor_set: P::DescriptorSet,
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
