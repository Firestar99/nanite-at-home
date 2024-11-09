use crate::backend::table::{FrameGuard, TableSync};
use crate::descriptor::buffer_table::{BufferTable, BufferTableAccess};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::image_table::{ImageTable, ImageTableAccess};
use crate::descriptor::sampler_table::{SamplerTable, SamplerTableAccess};
use crate::platform::BindlessPlatform;
use ash::vk::ShaderStageFlags;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

pub const BINDLESS_MAX_PUSH_CONSTANT_WORDS: usize = 4;

pub struct BindlessCreateInfo<P: BindlessPlatform> {
	pub instance: P::Instance,
	pub physical_device: P::PhysicalDevice,
	pub device: P::Device,
	pub memory_allocator: P::MemoryAllocator,
	pub counts: DescriptorCounts,
	pub shader_stages: ShaderStageFlags,
}

pub struct Bindless<P: BindlessPlatform> {
	pub ci: Arc<BindlessCreateInfo<P>>,
	/// always Some, Option is only needed for clean drop
	descriptor_set: Option<P::BindlessDescriptorSet>,
	// pipeline_layouts: [Arc<PipelineLayout>; BINDLESS_MAX_PUSH_CONSTANT_WORDS + 1],
	pub table_sync: Arc<TableSync>,
	pub(super) buffer: BufferTable<P>,
	pub(super) image: ImageTable<P>,
	pub(super) sampler: SamplerTable<P>,
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
	pub unsafe fn new(ci: Arc<BindlessCreateInfo<P>>) -> Arc<Self> {
		let limit = DescriptorCounts::limits(&ci);
		assert!(
			ci.counts.is_within_limit(limit),
			"counts {:?} must be within limit {:?}",
			ci.counts,
			limit
		);

		let descriptor_set = P::create_descriptor_set(&ci);

		let table_sync = TableSync::new();
		Arc::new(Self {
			buffer: BufferTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			image: ImageTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			sampler: SamplerTable::new(&table_sync, ci.clone(), descriptor_set.clone()),
			ci,
			table_sync,
			descriptor_set: Some(descriptor_set),
			// pipeline_layouts,
		})
	}

	#[inline(never)]
	fn unreachable_bindless_dropped() -> ! {
		unreachable!("Bindless has most likely been dropped");
	}

	#[inline]
	pub fn descriptor_set(&self) -> &P::BindlessDescriptorSet {
		match self.descriptor_set.as_ref() {
			None => Self::unreachable_bindless_dropped(),
			Some(set) => set,
		}
	}

	#[inline]
	pub fn table_sync(&self) -> &Arc<TableSync> {
		&self.table_sync
	}

	#[inline]
	pub fn buffer<'a>(self: &'a Arc<Self>) -> BufferTableAccess<'a, P> {
		BufferTableAccess(self)
	}

	#[inline]
	pub fn image<'a>(self: &'a Arc<Self>) -> ImageTableAccess<'a, P> {
		ImageTableAccess(self)
	}

	#[inline]
	pub fn sampler<'a>(self: &'a Arc<Self>) -> SamplerTableAccess<'a, P> {
		SamplerTableAccess(self)
	}

	/// Flush the bindless descriptor set. All newly allocated resources before this call will be written. Failing to
	/// flush before enqueueing work is undefined behaviour.
	pub fn flush(self: &Arc<Self>) {
		let flush_guard = self.table_sync.flush_lock();
		flush_guard.flush();
		unsafe {
			P::update_descriptor_set(
				&self.ci,
				self.descriptor_set(),
				self.buffer().flush_queue(),
				self.image().flush_queue(),
				self.sampler().flush_queue(),
			);
		}
	}

	/// Locking the Bindless system will ensure that any resource, that is dropped after the lock has been created, will
	/// not be deallocated or removed from the bindless descriptor set until this lock is dropped. There may be multiple
	/// active locks at the same time that can be unlocked out of order.
	#[inline]
	pub fn lock(&self) -> FrameGuard {
		self.table_sync.frame()
	}

	// /// Get a pipeline layout with just the bindless descriptor set and the correct push constant size  for your
	// /// `param_constant` `T`.
	// /// The push constant size must not exceed 4 words (of u32's), the minimum the device spec requires.
	// pub fn get_pipeline_layout<T: BufferStruct>(&self) -> Result<&Arc<PipelineLayout>, PushConstantError> {
	// 	let words = Self::get_push_constant_words::<T>();
	// 	self.pipeline_layouts
	// 		.get(words)
	// 		.ok_or(PushConstantError::TooLarge(words))
	// }
	//
	// /// Get a `Vec<PushConstantRange>` with the correct push constant size for your `param_constant` `T`.
	// /// The push constant size must not exceed 4 words (of u32's), the minimum the device spec requires.
	// pub fn get_push_constant<T: BufferStruct>(&self) -> Vec<PushConstantRange> {
	// 	Self::get_push_constant_inner(self.stages, Self::get_push_constant_words::<T>())
	// }
	//
	// fn get_push_constant_inner(stages: ShaderStages, words: usize) -> Vec<PushConstantRange> {
	// 	if words == 0 {
	// 		Vec::new()
	// 	} else {
	// 		Vec::from([PushConstantRange {
	// 			stages,
	// 			offset: 0,
	// 			size: words as u32 * 4,
	// 		}])
	// 	}
	// }
	//
	// /// Get the push constant word size (of u32's) for your `param_constant` `T`.
	// pub fn get_push_constant_words<T: BufferStruct>() -> usize {
	// 	let i = mem::size_of::<PushConstant<T::Transfer>>();
	// 	// round up to next multiple of words
	// 	(i + 3) / 4
	// }
}

impl<P: BindlessPlatform> Drop for Bindless<P> {
	fn drop(&mut self) {
		unsafe {
			P::destroy_descriptor_set(&self.ci, self.descriptor_set.take().unwrap());
		}
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
