use crate::backend::range_set::{range_to_descriptor_index, DescriptorIndexRangeSet};
use crate::backend::table::{DrainFlushQueue, RcTableSlot, Table, TableInterface, TableSync};
use crate::descriptor::descriptor_content::{DescContentCpu, DescTable};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::{Bindless, Image, RCDescExt};
use rust_gpu_bindless_shaders::descriptor::SampleType;
use rust_gpu_bindless_shaders::descriptor::{BINDING_SAMPLED_IMAGE, BINDING_STORAGE_IMAGE};
use rust_gpu_bindless_shaders::spirv_std::image::Image2d;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, InvalidateDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::image::view::{ImageView, ImageViewType};
use vulkano::image::ImageUsage;
use vulkano::shader::ShaderStages;

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
	type DescTable = ImageTable;
	type VulkanType = Arc<ImageView>;

	fn deref_table(slot: &RcTableSlot) -> &Self::VulkanType {
		slot.try_deref::<ImageInterface>().unwrap()
	}
}

impl DescTable for ImageTable {
	type Slot = Arc<ImageView>;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_sampled_images
			.unwrap()
	}

	fn layout_binding(
		stages: ShaderStages,
		count: DescriptorCounts,
		out: &mut BTreeMap<u32, DescriptorSetLayoutBinding>,
	) {
		out.insert(
			BINDING_STORAGE_IMAGE,
			DescriptorSetLayoutBinding {
				binding_flags: Self::BINDING_FLAGS,
				descriptor_count: count.image,
				stages,
				..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageImage)
			},
		)
		.ok_or(())
		.unwrap_err();
		out.insert(
			BINDING_SAMPLED_IMAGE,
			DescriptorSetLayoutBinding {
				binding_flags: Self::BINDING_FLAGS,
				descriptor_count: count.image,
				stages,
				..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::SampledImage)
			},
		)
		.ok_or(())
		.unwrap_err();
	}
}

pub struct ImageTable {
	table: Arc<Table<ImageInterface>>,
}

impl ImageTable {
	pub fn new(table_sync: &Arc<TableSync>, descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			table: table_sync.register(count, ImageInterface { descriptor_set }).unwrap(),
		}
	}
}

pub struct ImageTableAccess<'a>(pub &'a Arc<Bindless>);

impl<'a> Deref for ImageTableAccess<'a> {
	type Target = ImageTable;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0.image
	}
}

impl<'a> ImageTableAccess<'a> {
	#[inline]
	pub fn alloc_slot_2d(&self, image_view: Arc<ImageView>) -> RCDesc<Image2d> {
		unsafe {
			assert_eq!(image_view.view_type(), ImageViewType::Dim2d);
			RCDesc::new(
				self.table
					.alloc_slot(image_view)
					.map_err(|a| format!("ImageTable: {}", a))
					.unwrap(),
			)
		}
	}

	pub(crate) fn flush_descriptors(
		&self,
		delay_drop: &mut Vec<RcTableSlot>,
		mut writes: impl FnMut(WriteDescriptorSet),
	) {
		let flush_queue = self.table.drain_flush_queue();
		let mut storage = DescriptorIndexRangeSet::new();
		let mut sampled = DescriptorIndexRangeSet::new();
		delay_drop.reserve(flush_queue.size_hint().0);
		for x in flush_queue {
			let image = unsafe { self.table.get_slot_unchecked(x.id().index()) };
			if image.usage().contains(ImageUsage::STORAGE) {
				storage.insert(x.id().index());
			}
			if image.usage().contains(ImageUsage::SAMPLED) {
				sampled.insert(x.id().index());
			}
			delay_drop.push(x);
		}

		for (binding, range_set) in [(BINDING_STORAGE_IMAGE, storage), (BINDING_SAMPLED_IMAGE, sampled)] {
			for range in range_set.iter_ranges() {
				writes(WriteDescriptorSet::image_view_array(
					binding,
					range.start.to_u32(),
					range_to_descriptor_index(range)
						.map(|index| unsafe { self.table.get_slot_unchecked(index).clone() }),
				));
			}
		}
	}
}

pub struct ImageInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl TableInterface for ImageInterface {
	type Slot = Arc<ImageView>;

	fn drop_slots(&self, indices: &DescriptorIndexRangeSet) {
		for x in indices.iter() {
			self.descriptor_set
				.invalidate(&[
					InvalidateDescriptorSet::invalidate_array(BINDING_SAMPLED_IMAGE, x.to_u32(), 1),
					InvalidateDescriptorSet::invalidate_array(BINDING_STORAGE_IMAGE, x.to_u32(), 1),
				])
				.unwrap();
		}
	}

	fn flush(&self, _flush_queue: DrainFlushQueue<'_, Self>) {
		// do nothing, flushing of descriptors is handled differently
	}
}
