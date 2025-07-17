use crate::backing::table::{FrameGuard, TableSync};
use crate::descriptor::buffer_table::{BufferTable, BufferTableAccess};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::image_table::{ImageTable, ImageTableAccess};
use crate::descriptor::sampler_table::{SamplerTable, SamplerTableAccess};
use crate::platform::BindlessPlatform;
use rust_gpu_bindless_shaders::buffer_content::Metadata;
use rust_gpu_bindless_shaders::descriptor::TransientAccess;
use std::ops::Deref;
use std::sync::{Arc, Weak};

pub struct Bindless<P: BindlessPlatform>(Arc<BindlessInner<P>>);

impl<P: BindlessPlatform> Clone for Bindless<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P: BindlessPlatform> Deref for Bindless<P> {
	type Target = Arc<BindlessInner<P>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct WeakBindless<P: BindlessPlatform>(Weak<BindlessInner<P>>);

impl<P: BindlessPlatform> WeakBindless<P> {
	pub fn upgrade(&self) -> Option<Bindless<P>> {
		self.0.upgrade().map(Bindless)
	}
}

impl<P: BindlessPlatform> Clone for WeakBindless<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P: BindlessPlatform> Deref for WeakBindless<P> {
	type Target = Weak<BindlessInner<P>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct BindlessInner<P: BindlessPlatform> {
	pub platform: P,
	/// always Some, Option is only needed for clean drop
	descriptor_set: Option<P::BindlessDescriptorSet>,
	pub table_sync: Arc<TableSync>,
	pub(super) buffer: BufferTable<P>,
	pub(super) image: ImageTable<P>,
	pub(super) sampler: SamplerTable<P>,
}

impl<P: BindlessPlatform> Deref for BindlessInner<P> {
	type Target = P;

	fn deref(&self) -> &Self::Target {
		&self.platform
	}
}

/// Bindless will accept executions for as long as the initially returned [`BindlessInstance`] object is alive. When it
/// is dropped, the shutdown is initiated.
pub struct BindlessInstance<P: BindlessPlatform>(Bindless<P>);

impl<P: BindlessPlatform> Deref for BindlessInstance<P> {
	type Target = Bindless<P>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<P: BindlessPlatform> Drop for BindlessInstance<P> {
	fn drop(&mut self) {
		unsafe {
			self.bindless_shutdown(&self.0);
		}
	}
}

impl<P: BindlessPlatform> BindlessInstance<P> {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// * There must only be one global Bindless instance for each [`Device`].
	/// * The [general bindless safety requirements](crate#safety) apply
	pub unsafe fn new(ci: P::PlatformCreateInfo, counts: DescriptorCounts) -> Self {
		unsafe {
			let bindless = Bindless(Arc::new_cyclic(|weak| {
				let weak = WeakBindless(weak.clone());
				// TODO propagate error
				let platform = P::create_platform(ci, &weak).unwrap();
				counts.assert_within_limits::<P>(&platform);

				let table_sync = TableSync::new();
				BindlessInner {
					buffer: BufferTable::new(&table_sync, counts, weak.clone()),
					image: ImageTable::new(&table_sync, counts, weak.clone()),
					sampler: SamplerTable::new(&table_sync, counts, weak),
					descriptor_set: Some(platform.create_descriptor_set(counts)),
					table_sync,
					platform,
				}
			}));
			bindless.platform.bindless_initialized(&bindless);
			BindlessInstance(bindless)
		}
	}
}

impl<P: BindlessPlatform> Bindless<P> {
	#[inline(never)]
	fn unreachable_bindless_dropped() -> ! {
		unreachable!("Bindless has most likely been dropped");
	}

	#[inline]
	pub fn global_descriptor_set(&self) -> &P::BindlessDescriptorSet {
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
	pub fn buffer(&self) -> BufferTableAccess<'_, P> {
		BufferTableAccess(self)
	}

	#[inline]
	pub fn image(&self) -> ImageTableAccess<'_, P> {
		ImageTableAccess(self)
	}

	#[inline]
	pub fn sampler(&self) -> SamplerTableAccess<'_, P> {
		SamplerTableAccess(self)
	}

	/// Flush the bindless descriptor set. All newly allocated resources before this call will be written. Failing to
	/// flush before enqueueing work is undefined behaviour.
	pub fn flush(&self) {
		let flush_guard = self.table_sync.flush_lock();
		flush_guard.flush();
		unsafe {
			self.platform.update_descriptor_set(
				self.global_descriptor_set(),
				self.buffer().flush_queue(),
				self.image().flush_queue(),
				self.sampler().flush_queue(),
			);
		}
	}

	/// Creating a [`BindlessFrame`] will ensure that any resource, that is dropped after the lock has been created,
	/// will not be deallocated or removed from the bindless descriptor set until this lock is dropped. This allows
	/// creating [`TransientDesc`] from [`RCDesc`] which are guaranteed to be alive as long as [`BindlessFrame`] is.
	/// There may be multiple active Frames at the same time that can finish out of order.
	#[inline]
	pub fn frame(&self) -> BindlessFrame<P> {
		BindlessFrame(Arc::new(BindlessFrameInner {
			bindless: self.clone(),
			frame_guard: self.table_sync.frame(),
			metadata: Metadata,
		}))
	}
}

impl<P: BindlessPlatform> Drop for BindlessInner<P> {
	fn drop(&mut self) {
		unsafe {
			self.platform
				.destroy_descriptor_set(self.descriptor_set.take().unwrap());
		}
	}
}

pub struct BindlessFrame<P: BindlessPlatform>(Arc<BindlessFrameInner<P>>);

impl<P: BindlessPlatform> Clone for BindlessFrame<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P: BindlessPlatform> Deref for BindlessFrame<P> {
	type Target = Arc<BindlessFrameInner<P>>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct BindlessFrameInner<P: BindlessPlatform> {
	pub bindless: Bindless<P>,
	pub frame_guard: FrameGuard,
	pub metadata: Metadata,
}

unsafe impl<'a, P: BindlessPlatform> TransientAccess<'a> for &'a BindlessFrame<P> {}
