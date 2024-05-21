use crate::descriptor::bindless_descriptor_allocator::BindlessDescriptorSetAllocator;
use crate::descriptor::buffer_table::BufferTable;
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::descriptor_type_cpu::DescTable;
use crate::descriptor::image_table::ImageTable;
use crate::descriptor::resource_table::Lock;
use crate::descriptor::sampler_table::SamplerTable;
use crate::sync::Mutex;
use smallvec::SmallVec;
use static_assertions::assert_impl_all;
use std::array;
use std::collections::BTreeMap;
use std::sync::Arc;
use vulkano::descriptor_set::layout::{
	DescriptorSetLayout, DescriptorSetLayoutCreateFlags, DescriptorSetLayoutCreateInfo,
};
use vulkano::descriptor_set::DescriptorSet;
use vulkano::device::Device;
use vulkano::pipeline::layout::{PipelineLayoutCreateInfo, PushConstantRange};
use vulkano::pipeline::PipelineLayout;
use vulkano::shader::ShaderStages;

pub const BINDLESS_PIPELINE_LAYOUT_PUSH_CONSTANT_WORDS: usize = 5;

pub struct Bindless {
	pub device: Arc<Device>,
	pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	pipeline_layouts: [Arc<PipelineLayout>; BINDLESS_PIPELINE_LAYOUT_PUSH_CONSTANT_WORDS],
	pub descriptor_set: Arc<DescriptorSet>,
	pub buffer: BufferTable,
	pub image: ImageTable,
	pub sampler: SamplerTable,
	flush_lock: Mutex<()>,
}

pub struct BindlessLock {
	_buffer: Lock<BufferTable>,
	_image: Lock<ImageTable>,
	_sampler: Lock<SamplerTable>,
}

assert_impl_all!(Bindless: Send, Sync);

impl Bindless {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// * There must only be one global Bindless instance for each [`Device`].
	/// * The [general bindless safety requirements](crate#safety) apply
	pub unsafe fn new(device: Arc<Device>, stages: ShaderStages, counts: DescriptorCounts) -> Arc<Self> {
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

		let pipeline_layouts = array::from_fn(|i| {
			PipelineLayout::new(
				device.clone(),
				PipelineLayoutCreateInfo {
					set_layouts: Vec::from([descriptor_set_layout.clone()]),
					push_constant_ranges: if i == 0 {
						Vec::new()
					} else {
						Vec::from([PushConstantRange {
							stages,
							offset: 0,
							size: i as u32 * 4,
						}])
					},
					..PipelineLayoutCreateInfo::default()
				},
			)
			.unwrap()
		});

		Arc::new(Self {
			buffer: BufferTable::new(descriptor_set.clone(), counts.buffers),
			image: ImageTable::new(descriptor_set.clone(), counts.image),
			sampler: SamplerTable::new(descriptor_set.clone(), counts.samplers),
			descriptor_set_layout,
			pipeline_layouts,
			descriptor_set,
			device,
			flush_lock: Mutex::new(()),
		})
	}

	/// Flush the bindless descriptor set. All newly allocated resources before this call will be written.
	/// Instead of manual flushing, one should prefer to use [`FrameManager`]'s flushing capabilities.
	pub fn flush(&self) {
		// flushes must be sequential. Finer sync may be possible, but probably not worth it.
		let _flush_guard = self.flush_lock.lock();

		// Safety: update-after-bind descriptors have relaxed external synchronization requirements:
		//	* only one thread may update at once, ensured by flush_queue Mutex
		//  * descriptor set may be used in command buffers concurrently, see spec
		unsafe {
			let mut writes: SmallVec<[_; 8]> = SmallVec::new();
			let buffer = self.buffer.flush_updates(&mut writes);
			let image = self.image.flush_updates(&mut writes);
			let sampler = self.sampler.flush_updates(&mut writes);
			if !writes.is_empty() {
				self.descriptor_set.update_by_ref(writes, []).unwrap();
			}
			// drop after update, to allow already freed slots to correctly invalidate the descriptor slot
			drop((buffer, image, sampler));
		}
	}

	/// Locking the Bindless system will ensure that any resource, that is dropped after the lock has been created, will
	/// not be deallocated or removed from the bindless descriptor set until this lock is dropped. There may be multiple
	/// active locks at the same time that can be unlocked out of order.
	pub fn lock(&self) -> BindlessLock {
		BindlessLock {
			_buffer: self.buffer.lock_table(),
			_image: self.image.lock_table(),
			_sampler: self.sampler.lock_table(),
		}
	}

	/// Get a pipeline layout with just the bindless descriptor set and some amount of push constant words (of u32's).
	/// `push_constant_words` must be within `0..=4`, the minimum the vulkan spec requires.
	pub fn get_pipeline_layout(&self, push_constant_words: u32) -> Option<&Arc<PipelineLayout>> {
		self.pipeline_layouts.get(push_constant_words as usize)
	}
}
