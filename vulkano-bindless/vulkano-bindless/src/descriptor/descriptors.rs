use crate::descriptor::bindless_descriptor_allocator::BindlessDescriptorSetAllocator;
use crate::descriptor::buffer_table::BufferResourceTable;
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::ResourceTableCpu;
use crate::descriptor::image_table::ImageResourceTable;
use crate::descriptor::sampler_table::SamplerResourceTable;
use crate::sync::Mutex;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutCreateFlags, DescriptorSetLayoutCreateInfo,
};
use vulkano::descriptor_set::DescriptorSet;
use vulkano::device::Device;
use vulkano::shader::ShaderStages;
use vulkano_bindless_shaders::descriptor::buffer::BufferTable;
use vulkano_bindless_shaders::descriptor::{ImageTable, SamplerTable};

pub struct DescriptorsCpu {
	pub device: Arc<Device>,
	pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	pub descriptor_set: Arc<DescriptorSet>,
	pub buffer: BufferResourceTable,
	pub image: ImageResourceTable,
	pub sampler: SamplerResourceTable,
	flush_lock: Mutex<()>,
}

impl DescriptorsCpu {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// There must only be one global Descriptors instance for each [`Device`].
	/// Before executing commands, one must [`Self::flush`] the Descriptors.
	pub unsafe fn new(device: Arc<Device>, stages: ShaderStages, counts: DescriptorCounts) -> Self {
		let limit = DescriptorCounts::limits(device.physical_device());
		assert!(
			counts.is_within_limit(limit),
			"counts {:?} must be within limit {:?}",
			counts,
			limit
		);

		let mut bindings = BTreeMap::new();
		BufferTable::layout_binding(stages, counts, &mut bindings);
		SamplerTable::layout_binding(stages, counts, &mut bindings);
		ImageTable::layout_binding(stages, counts, &mut bindings);

		let descriptor_set_layout = DescriptorSetLayout::new(
			device.clone(),
			DescriptorSetLayoutCreateInfo {
				flags: DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
				bindings,
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
			flush_lock: Mutex::new(()),
		}
	}

	pub fn flush(&self) {
		// flushes must be sequential. Finer sync may be possible, but probably not worth it.
		let _flush_guard = self.flush_lock.lock();

		// Safety: update-after-bind descriptors have relaxed external synchronization requirements:
		//	* only one thread may update at once, ensured by flush_queue Mutex
		//  * descriptor set may be used in command buffers concurrently, see spec
		unsafe {
			let mut writes: SmallVec<[_; 8]> = SmallVec::new();
			self.buffer.flush_updates(&mut writes);
			self.image.flush_updates(&mut writes);
			self.sampler.flush_updates(&mut writes);
			if !writes.is_empty() {
				self.descriptor_set.update_by_ref(writes, []).unwrap();
			}
		}
	}
}
