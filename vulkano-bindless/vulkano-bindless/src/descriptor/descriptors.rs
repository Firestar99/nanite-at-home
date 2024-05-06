use crate::descriptor::bindless_descriptor_allocator::BindlessDescriptorSetAllocator;
use crate::descriptor::buffer_table::BufferResourceTable;
use crate::descriptor::descriptor_type_cpu::ResourceTableCpu;
use crate::descriptor::image_table::ImageResourceTable;
use crate::descriptor::resource_table::ResourceTable;
use crate::descriptor::sampler_table::SamplerResourceTable;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutCreateFlags, DescriptorSetLayoutCreateInfo,
};
use vulkano::descriptor_set::DescriptorSet;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::buffer::BufferTable;
use vulkano_bindless_shaders::descriptor::{ImageTable, SamplerTable};

pub struct DescriptorCounts {
	pub buffers: u32,
	pub image: u32,
	pub samplers: u32,
}

impl DescriptorCounts {
	pub fn limits(phy: &Arc<PhysicalDevice>) -> Self {
		Self {
			buffers: BufferTable::max_update_after_bind_descriptors(phy),
			image: ImageTable::max_update_after_bind_descriptors(phy),
			samplers: SamplerTable::max_update_after_bind_descriptors(phy),
		}
	}

	pub fn reasonable_defaults(phy: &Arc<PhysicalDevice>) -> Self {
		let limits = Self::limits(phy);
		// These reasonable limits are copied from Daxa
		Self {
			buffers: limits.buffers.min(10_000),
			image: limits.image.min(10_000),
			samplers: limits.samplers.min(400),
		}
	}
}

pub struct DescriptorsCpu {
	pub device: Arc<Device>,
	pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	pub descriptor_set: Arc<DescriptorSet>,
	pub buffer: BufferResourceTable,
	pub image: ImageResourceTable,
	pub sampler: SamplerResourceTable,
	_private: PhantomData<()>,
}

impl DescriptorsCpu {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// There must only be one global Descriptors instance for each [`Device`].
	/// Before executing commands, one must [`Self::flush`] the Descriptors.
	pub unsafe fn new(device: Arc<Device>, stages: ShaderStages, counts: DescriptorCounts) -> Self {
		let descriptor_set_layout = DescriptorSetLayout::new(
			device.clone(),
			DescriptorSetLayoutCreateInfo {
				flags: DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
				bindings: BTreeMap::from([
					ResourceTable::<BufferTable>::layout_binding(&device, stages, counts.buffers),
					ResourceTable::<SamplerTable>::layout_binding(&device, stages, counts.samplers),
					ResourceTable::<ImageTable>::layout_binding(&device, stages, counts.image),
				]),
				..DescriptorSetLayoutCreateInfo::default()
			},
		)
		.unwrap();
		let allocator = BindlessDescriptorSetAllocator::new(device.clone());
		let descriptor_set = DescriptorSet::new(allocator, descriptor_set_layout.clone(), [], []).unwrap();

		Self {
			descriptor_set_layout,
			descriptor_set,
			buffer: BufferResourceTable::new(device.clone(), counts.buffers),
			image: ImageResourceTable::new(device.clone(), counts.image),
			sampler: SamplerResourceTable::new(device.clone(), counts.samplers),
			device,
			_private: PhantomData {},
		}
	}

	pub fn flush(&self) {
		// Safety: update-after-bind descriptors have relaxed external synchronization requirements:
		//	* only one thread may update at once, ensured by flush_queue Mutex
		//  * descriptor set may be used in command buffers concurrently, see spec
		unsafe {
			let mut writes: SmallVec<[_; 8]> = SmallVec::new();
			self.buffer.resource_table.flush_updates(&mut writes);
			self.image.resource_table.flush_updates(&mut writes);
			self.sampler.resource_table.flush_updates(&mut writes);
			if !writes.is_empty() {
				self.descriptor_set.update_by_ref(writes, []).unwrap();
			}
		}
	}
}
