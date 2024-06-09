use crate::descriptor::descriptor_content::{DescContentCpu, DescTable, DescTableEnum, DescTableEnumType};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::{FlushUpdates, ResourceTable, TableEpochGuard};
use crate::descriptor::{Bindless, Image};
use crate::rc_slot::{RCSlotsInterface, SlotIndex};
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorType};
use vulkano::descriptor_set::{DescriptorSet, InvalidateDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Device, DeviceOwned};
use vulkano::image::view::{ImageView, ImageViewType};
use vulkano::image::ImageUsage;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::descriptor_content::DescContentEnum;
use vulkano_bindless_shaders::descriptor::image::SampleType;
use vulkano_bindless_shaders::descriptor::{BINDING_SAMPLED_IMAGE, BINDING_STORAGE_IMAGE};
use vulkano_bindless_shaders::spirv_std::image::Image2d;

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

	fn deref_table(slot: &<Self::DescTable as DescTable>::Slot) -> &Self::VulkanType {
		slot
	}
}

impl DescTable for ImageTable {
	const CONTENT_ENUM: DescContentEnum = DescContentEnum::Image;
	type Slot = Arc<ImageView>;
	type RCSlotsInterface = ImageInterface;

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

	#[inline]
	fn lock_table(&self) -> TableEpochGuard<Self> {
		self.resource_table.epoch_guard()
	}

	#[inline]
	fn table_enum_new<A: DescTableEnumType>(inner: A::Type<Self>) -> DescTableEnum<A> {
		DescTableEnum::Image(inner)
	}

	#[inline]
	fn table_enum_try_deref<A: DescTableEnumType>(table_enum: &DescTableEnum<A>) -> Option<&A::Type<Self>> {
		if let DescTableEnum::Image(v) = table_enum {
			Some(v)
		} else {
			None
		}
	}

	#[inline]
	fn table_enum_try_into<A: DescTableEnumType>(
		table_enum: DescTableEnum<A>,
	) -> Result<A::Type<Self>, DescTableEnum<A>> {
		if let DescTableEnum::Image(v) = table_enum {
			Ok(v)
		} else {
			Err(table_enum)
		}
	}
}

pub struct ImageTable {
	pub device: Arc<Device>,
	pub(super) resource_table: ResourceTable<ImageTable>,
}

impl ImageTable {
	pub fn new(descriptor_set: Arc<DescriptorSet>, count: u32) -> Self {
		Self {
			device: descriptor_set.device().clone(),
			resource_table: ResourceTable::new(count, ImageInterface { descriptor_set }),
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
		assert_eq!(image_view.view_type(), ImageViewType::Dim2d);
		self.resource_table.alloc_slot(image_view)
	}
}

impl ImageTable {
	pub(crate) fn flush_updates(&self, mut writes: impl FnMut(WriteDescriptorSet)) -> FlushUpdates<ImageTable> {
		// TODO writes is written out-of-order with regard to bindings.
		//   Would it be worth to buffer all writes of one binding, only flushing at the end?

		let mut storage_buf = ImageUpdateBuffer::new(BINDING_STORAGE_IMAGE);
		let mut sampled_buf = ImageUpdateBuffer::new(BINDING_SAMPLED_IMAGE);
		let flush_updates = self.resource_table.flush_updates();
		flush_updates.iter(|start, buffer| {
			storage_buf.start(start, buffer.capacity());
			sampled_buf.start(start, buffer.capacity());

			for image in buffer.iter() {
				let sampled = image.usage().contains(ImageUsage::SAMPLED);
				let storage = image.usage().contains(ImageUsage::STORAGE);
				match (storage, sampled) {
					(true, true) => {
						storage_buf.advance_push(image.clone());
						sampled_buf.advance_push(image.clone());
					}
					(true, false) => {
						storage_buf.advance_push(image.clone());
						sampled_buf.advance_flush(&mut writes);
					}
					(false, true) => {
						storage_buf.advance_flush(&mut writes);
						sampled_buf.advance_push(image.clone());
					}
					(false, false) => {
						storage_buf.advance_flush(&mut writes);
						sampled_buf.advance_flush(&mut writes);
					}
				}
			}

			storage_buf.advance_flush(&mut writes);
			sampled_buf.advance_flush(&mut writes);
		});
		flush_updates
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

	fn advance_flush(&mut self, writes: &mut impl FnMut(WriteDescriptorSet)) {
		let len = self.buffer.len() as u32;
		if len > 0 {
			writes(WriteDescriptorSet::image_view_array(
				self.binding,
				self.start,
				self.buffer.drain(..),
			));
		}
		self.start += len + 1;
	}
}

pub struct ImageInterface {
	descriptor_set: Arc<DescriptorSet>,
}

impl RCSlotsInterface<<ImageTable as DescTable>::Slot> for ImageInterface {
	fn drop_slot(&self, index: SlotIndex, image: <ImageTable as DescTable>::Slot) {
		let sampled = image.usage().contains(ImageUsage::SAMPLED);
		let storage = image.usage().contains(ImageUsage::STORAGE);
		let mut invalidates: SmallVec<[_; 2]> = SmallVec::new();
		if sampled {
			invalidates.push(InvalidateDescriptorSet::invalidate_array(
				BINDING_SAMPLED_IMAGE,
				index.0 as u32,
				1,
			));
		}
		if storage {
			invalidates.push(InvalidateDescriptorSet::invalidate_array(
				BINDING_STORAGE_IMAGE,
				index.0 as u32,
				1,
			));
		}
		self.descriptor_set.invalidate(&invalidates).unwrap();
		drop(image);
	}
}
