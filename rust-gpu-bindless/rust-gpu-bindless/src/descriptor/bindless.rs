use crate::backing::table::{FrameGuard, TableSync};
use crate::descriptor::buffer_table::{BufferTable, BufferTableAccess};
use crate::descriptor::descriptor_counts::DescriptorCounts;
use crate::descriptor::image_table::{ImageTable, ImageTableAccess};
use crate::descriptor::sampler_table::{SamplerTable, SamplerTableAccess};
use crate::platform::BindlessPlatform;
use std::ops::Deref;
use std::sync::Arc;

pub struct Bindless<P: BindlessPlatform> {
	pub platform: P,
	/// always Some, Option is only needed for clean drop
	descriptor_set: Option<P::BindlessDescriptorSet>,
	pub table_sync: Arc<TableSync>,
	pub(super) buffer: BufferTable<P>,
	pub(super) image: ImageTable<P>,
	pub(super) sampler: SamplerTable<P>,
}

impl<P: BindlessPlatform> Deref for Bindless<P> {
	type Target = P;

	fn deref(&self) -> &Self::Target {
		&self.platform
	}
}

impl<P: BindlessPlatform> Bindless<P> {
	/// Creates a new Descriptors instance with which to allocate descriptors.
	///
	/// # Safety
	/// * There must only be one global Bindless instance for each [`Device`].
	/// * The [general bindless safety requirements](crate#safety) apply
	pub unsafe fn new(ci: P::PlatformCreateInfo, counts: DescriptorCounts) -> Arc<Self> {
		Arc::new_cyclic(|weak| {
			let platform = P::create_platform(ci, weak);
			counts.assert_within_limits::<P>(&platform);

			let table_sync = TableSync::new();
			Self {
				buffer: BufferTable::new(&table_sync, counts, weak.clone()),
				image: ImageTable::new(&table_sync, counts, weak.clone()),
				sampler: SamplerTable::new(&table_sync, counts, weak.clone()),
				descriptor_set: Some(platform.create_descriptor_set(counts)),
				table_sync,
				platform,
			}
		})
	}

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
			self.platform.update_descriptor_set(
				self.global_descriptor_set(),
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
	pub fn frame(&self) -> FrameGuard {
		self.table_sync.frame()
	}
}

impl<P: BindlessPlatform> Drop for Bindless<P> {
	fn drop(&mut self) {
		unsafe {
			self.platform
				.destroy_descriptor_set(self.descriptor_set.take().unwrap());
		}
	}
}
