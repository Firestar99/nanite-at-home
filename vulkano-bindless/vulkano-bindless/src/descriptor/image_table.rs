use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::descriptor::resource_table::ResourceTable;
use crate::rc_slots::RCSlot;
use std::sync::Arc;
use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::image::view::{ImageView, ImageViewType};
use vulkano::image::ImageUsage;
use vulkano_bindless_shaders::descriptor::{ImageTable, SampledImage2D};

impl DescTypeCpu for SampledImage2D {
	type ResourceTableCpu = ImageTable;
	type CpuType = Arc<ImageView>;

	fn deref_table(slot: &RCSlot<<Self::ResourceTableCpu as ResourceTableCpu>::SlotType>) -> &Self::CpuType {
		slot
	}

	fn to_table(from: Self::CpuType) -> <Self::ResourceTableCpu as ResourceTableCpu>::SlotType {
		let from: Arc<ImageView> = from;
		assert!(from.usage().contains(ImageUsage::SAMPLED));
		assert_eq!(from.view_type(), ImageViewType::Dim2d);
		from
	}
}

impl ResourceTableCpu for ImageTable {
	type SlotType = Arc<ImageView>;
	const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::SampledImage;

	fn max_update_after_bind_descriptors(physical_device: &Arc<PhysicalDevice>) -> u32 {
		physical_device
			.properties()
			.max_descriptor_set_update_after_bind_sampled_images
			.unwrap()
	}

	fn write_descriptor_set(
		binding: u32,
		first_array_element: u32,
		elements: impl IntoIterator<Item = Self::SlotType>,
	) -> WriteDescriptorSet {
		WriteDescriptorSet::image_view_array(binding, first_array_element, elements)
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
}
