use crate::backend::table::{FrameGuard, TableSync};
use crate::descriptor::buffer_table::{BufferTable, BufferTableAccess};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::image_table::{ImageTable, ImageTableAccess};
use crate::descriptor::sampler_table::{SamplerTable, SamplerTableAccess};
use crate::platform::BindlessPlatform;
use parking_lot::Mutex;
use rust_gpu_bindless_shaders::buffer_content::BufferStruct;
use rust_gpu_bindless_shaders::descriptor::PushConstant;
use smallvec::SmallVec;
use std::fmt::{Display, Formatter};
use std::mem;
use std::ops::Deref;
use std::sync::Arc;

pub const BINDLESS_MAX_PUSH_CONSTANT_WORDS: usize = 4;

pub struct BindlessCreateInfo<P: BindlessPlatform> {
	pub instance: P::Instance,
	pub physical_device: P::PhysicalDevice,
	pub device: P::Device,
	pub memory_allocator: P::MemoryAllocator,
	pub counts: DescriptorCounts,
}

pub struct Bindless<P: BindlessPlatform> {
	ci: Arc<BindlessCreateInfo<P>>,
	// pub stages: ShaderStages,
	// pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	// pipeline_layouts: [Arc<PipelineLayout>; BINDLESS_MAX_PUSH_CONSTANT_WORDS + 1],
	// pub descriptor_set: Arc<DescriptorSet>,
	pub table_sync: Arc<TableSync>,
	pub(super) buffer: BufferTable<P>,
	pub(super) image: ImageTable<P>,
	pub(super) sampler: SamplerTable<P>,
	flush_lock: Mutex<()>,
}

impl<P: BindlessPlatform> Deref for Bindless<P> {
	type Target = BindlessCreateInfo<P>;

	fn deref(&self) -> &Self::Target {
		&self.ci
	}
}

impl<P: BindlessPlatform> Bindless<P> {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// * There must only be one global Bindless instance for each [`Device`].
	/// * The [general bindless safety requirements](crate#safety) apply
	pub unsafe fn new(ci: BindlessCreateInfo<P>) -> Arc<Self> {
		let ci = Arc::new(ci);
		let limit = DescriptorCounts::limits(&ci);
		assert!(
			ci.counts.is_within_limit(limit),
			"counts {:?} must be within limit {:?}",
			ci.counts,
			limit
		);

		// let mut bindings = BTreeMap::new();
		// BufferTable::layout_binding(stages, counts, &mut bindings);
		// SamplerTable::layout_binding(stages, counts, &mut bindings);
		// ImageTable::layout_binding(stages, counts, &mut bindings);
		//
		// let descriptor_set_layout = DescriptorSetLayout::new(
		// 	device.clone(),
		// 	DescriptorSetLayoutCreateInfo {
		// 		flags: DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
		// 		bindings,
		// 		..DescriptorSetLayoutCreateInfo::default()
		// 	},
		// )
		// .unwrap();
		// let allocator = BindlessDescriptorSetAllocator::new(device.clone());
		// let descriptor_set = DescriptorSet::new(allocator, descriptor_set_layout.clone(), [], []).unwrap();
		//
		// let pipeline_layouts = array::from_fn(|i| {
		// 	PipelineLayout::new(
		// 		device.clone(),
		// 		PipelineLayoutCreateInfo {
		// 			set_layouts: Vec::from([descriptor_set_layout.clone()]),
		// 			push_constant_ranges: Self::get_push_constant_inner(stages, i),
		// 			..PipelineLayoutCreateInfo::default()
		// 		},
		// 	)
		// 	.unwrap()
		// });
		let descriptor_set = todo!();

		let table_sync = TableSync::new();
		Arc::new(Self {
			buffer: BufferTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			image: ImageTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			sampler: SamplerTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			ci,
			table_sync,
			// descriptor_set_layout,
			// pipeline_layouts,
			// descriptor_set,
			// stages,
			flush_lock: Mutex::new(()),
		})
	}

	pub fn buffer<'a>(self: &'a Arc<Self>) -> BufferTableAccess<'a, P> {
		BufferTableAccess(self)
	}

	pub fn image<'a>(self: &'a Arc<Self>) -> ImageTableAccess<'a, P> {
		ImageTableAccess(self)
	}

	pub fn sampler<'a>(self: &'a Arc<Self>) -> SamplerTableAccess<'a, P> {
		SamplerTableAccess(self)
	}

	/// Flush the bindless descriptor set. All newly allocated resources before this call will be written.
	/// Instead of manual flushing, one should prefer to use [`FrameManager`]'s flushing capabilities.
	pub fn flush(self: &Arc<Self>) {
		// flushes must be sequential. Finer sync may be possible, but probably not worth it.
		let _flush_guard = self.flush_lock.lock();

		// Safety: update-after-bind descriptors have relaxed external synchronization requirements:
		//	* only one thread may update at once, ensured by flush_queue Mutex
		//  * descriptor set may be used in command buffers concurrently, see spec
		unsafe {
			let mut writes: SmallVec<[_; 8]> = SmallVec::new();
			let mut delay_drop = Vec::new();
			self.buffer().flush_descriptors(&mut delay_drop, |w| writes.push(w));
			self.image().flush_descriptors(&mut delay_drop, |w| writes.push(w));
			self.sampler().flush_descriptors(&mut delay_drop, |w| writes.push(w));
			if !writes.is_empty() {
				self.descriptor_set.update_by_ref(writes, []).unwrap();
			}
			// drop after update, to allow already freed slots to correctly invalidate the descriptor slot
			drop(delay_drop);
		}
	}

	/// Locking the Bindless system will ensure that any resource, that is dropped after the lock has been created, will
	/// not be deallocated or removed from the bindless descriptor set until this lock is dropped. There may be multiple
	/// active locks at the same time that can be unlocked out of order.
	pub fn lock(&self) -> FrameGuard {
		self.table_sync.frame()
	}

	/// Get a pipeline layout with just the bindless descriptor set and the correct push constant size  for your
	/// `param_constant` `T`.
	/// The push constant size must not exceed 4 words (of u32's), the minimum the device spec requires.
	pub fn get_pipeline_layout<T: BufferStruct>(&self) -> Result<&Arc<PipelineLayout>, PushConstantError> {
		let words = Self::get_push_constant_words::<T>();
		self.pipeline_layouts
			.get(words)
			.ok_or(PushConstantError::TooLarge(words))
	}

	/// Get a `Vec<PushConstantRange>` with the correct push constant size for your `param_constant` `T`.
	/// The push constant size must not exceed 4 words (of u32's), the minimum the device spec requires.
	pub fn get_push_constant<T: BufferStruct>(&self) -> Vec<PushConstantRange> {
		Self::get_push_constant_inner(self.stages, Self::get_push_constant_words::<T>())
	}

	fn get_push_constant_inner(stages: ShaderStages, words: usize) -> Vec<PushConstantRange> {
		if words == 0 {
			Vec::new()
		} else {
			Vec::from([PushConstantRange {
				stages,
				offset: 0,
				size: words as u32 * 4,
			}])
		}
	}

	/// Get the push constant word size (of u32's) for your `param_constant` `T`.
	pub fn get_push_constant_words<T: BufferStruct>() -> usize {
		let i = mem::size_of::<PushConstant<T::Transfer>>();
		// round up to next multiple of words
		(i + 3) / 4
	}
}

#[derive(Debug)]
pub enum PushConstantError {
	TooLarge(usize),
}

impl Display for PushConstantError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			PushConstantError::TooLarge(words) => f.write_fmt(format_args!(
				"Bindless param T of word size {} is too large for minimum device spec of 4",
				words
			)),
		}
	}
}
