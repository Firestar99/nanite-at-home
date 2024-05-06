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
use vulkano::image::view::{ImageView, ImageViewType};
use vulkano::image::ImageUsage;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::{ImageTable, SampledImage2D, BINDING_SAMPLED_IMAGE, BINDING_STORAGE_IMAGE};

impl DescTypeCpu for SampledImage2D {
	type ResourceTableCpu = ImageTable;
	type CpuType = Arc<ImageView>;

	fn deref_table(slot: &RCSlot<<Self::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> &Self::CpuType {
		slot
	}

	fn to_table(from: Self::CpuType) -> <Self::ResourceTableCpu as ResourceTableCpu>::SlotType {
		let from: Arc<ImageView> = from;
		assert_eq!(from.view_type(), ImageViewType::Dim2d);
		from
	}
}

impl ResourceTableCpu for ImageTable {
	type SlotType = Arc<ImageView>;

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

pub struct ImageResourceTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<ImageTable>,
}

impl ImageResourceTable {
	pub fn new(device: Arc<Device>, count: u32) -> Self {
		Self {
			device,
			resource_table: ResourceTable::new(count),
		}
	}

	#[inline]
	pub fn alloc_slot(&self, image_view: Arc<ImageView>) -> RCDesc<SampledImage2D> {
		self.resource_table.alloc_slot(image_view)
	}

	pub(crate) fn flush_updates<const C: usize>(&self, writes: &mut SmallVec<[WriteDescriptorSet; C]>) {
		// TODO writes is written out-of-order with regard to bindings.
		//   Would it be worth to buffer all writes of one binding, only flushing at the end?

		let mut storage_buf = ImageUpdateBuffer::new(BINDING_STORAGE_IMAGE);
		let mut sampled_buf = ImageUpdateBuffer::new(BINDING_SAMPLED_IMAGE);
		self.resource_table.flush_updates(|start, buffer| {
			storage_buf.start(start, buffer.capacity());
			sampled_buf.start(start, buffer.capacity());

			for image in buffer.drain(..) {
				let sampled = image.usage().contains(ImageUsage::SAMPLED);
				let storage = image.usage().contains(ImageUsage::STORAGE);
				match (storage, sampled) {
					(true, true) => {
						storage_buf.advance_push(image.clone());
						sampled_buf.advance_push(image);
					}
					(true, false) => {
						storage_buf.advance_push(image);
						sampled_buf.advance_flush(writes);
					}
					(false, true) => {
						storage_buf.advance_flush(writes);
						sampled_buf.advance_push(image);
					}
					(false, false) => {
						drop(image);
						storage_buf.advance_flush(writes);
						sampled_buf.advance_flush(writes);
					}
				}
			}

			storage_buf.advance_flush(writes);
			sampled_buf.advance_flush(writes);
		})
	}
}

struct ImageUpdateBuffer {
	binding: u32,
	start: u32,
	buffer: Vec<Arc<ImageView>>,
}

impl ImageUpdateBuffer {
	const fn new(binding: u32) -> Self {
		Self {
			buffer: Vec::new(),
			binding,
			start: !0,
		}
	}

	fn start(&mut self, start: u32, capacity: usize) {
		self.start = start;
		self.buffer.reserve_exact(capacity);
	}

	fn advance_push(&mut self, instance: Arc<ImageView>) {
		self.buffer.push(instance)
	}

	fn advance_flush(&mut self, writes: &mut impl Extend<WriteDescriptorSet>) {
		let len = self.buffer.len() as u32;
		if len > 0 {
			writes.extend([WriteDescriptorSet::image_view_array(
				self.binding,
				self.start,
				self.buffer.drain(..),
			)]);
		}
		self.start += len + 1;
	}
}
