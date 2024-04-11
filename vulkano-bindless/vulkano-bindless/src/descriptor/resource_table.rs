use crate::atomic_slots::{AtomicRCSlots, AtomicRCSlotsLock, RCSlot};
use crate::descriptor::bindless_descriptor_allocator::BindlessDescriptorSetAllocator;
use crate::descriptor::descriptor_type_cpu::{DescTypeCpu, ResourceTableCpu};
use crate::descriptor::rc_reference::RCDesc;
use crate::sync::SpinWait;
use arc_swap::{ArcSwap, ArcSwapOption};
use crossbeam_queue::SegQueue;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::collections::BTreeMap;
use std::mem;
use std::ops::Deref;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicUsize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use vulkano::descriptor_set::allocator::DescriptorSetAllocator;
use vulkano::descriptor_set::layout::{
	DescriptorBindingFlags, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
	DescriptorSetLayoutCreateInfo,
};
use vulkano::descriptor_set::sys::RawDescriptorSet;
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::shader::ShaderStages;

pub const SLOTS_FIRST_BLOCK_SIZE: u32 = 128;
pub const REALLOCATION_OVERALLOCATION_FACTOR: f32 = 0.5;

pub struct ResourceTable<T: ResourceTableCpu> {
	pub device: Arc<Device>,
	pub descriptor_set_layout: Arc<DescriptorSetLayout>,
	descriptor_set_allocator: Arc<dyn DescriptorSetAllocator>,
	slots: Arc<AtomicRCSlots<T::SlotType>>,
	queue: SegQueue<RCSlot<T::SlotType>>,
	inner: Mutex<Inner>,
	descriptor_capacity: AtomicU32,
	descriptor: ArcSwapOption<DescriptorSet>,
}

struct Inner {
	/// when flushing, incremental writing is not enough, a full write of all valid descriptors is required
	full_write_required: bool,
	old_descriptors: SmallVec<[Arc<DescriptorSet>; 1]>,
}

impl<T: ResourceTableCpu> ResourceTable<T> {
	pub fn new(
		device: Arc<Device>,
		stages: ShaderStages,
		descriptor_set_allocator: Arc<dyn DescriptorSetAllocator>,
	) -> Self {
		let descriptor_set_layout = DescriptorSetLayout::new(
			device.clone(),
			DescriptorSetLayoutCreateInfo {
				flags: DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL,
				bindings: BTreeMap::from([(
					0,
					DescriptorSetLayoutBinding {
						binding_flags: DescriptorBindingFlags::UPDATE_AFTER_BIND
							| DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING
							| DescriptorBindingFlags::PARTIALLY_BOUND
							| DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT,
						descriptor_count: T::max_update_after_bind_descriptors(device.physical_device()),
						stages,
						..DescriptorSetLayoutBinding::descriptor_type(T::DESCRIPTOR_TYPE)
					},
				)]),
				..DescriptorSetLayoutCreateInfo::default()
			},
		)
		.unwrap();

		Self {
			slots: AtomicRCSlots::new(SLOTS_FIRST_BLOCK_SIZE),
			descriptor_set_layout,
			descriptor_set_allocator,
			queue: SegQueue::new(),
			inner: Mutex::new(Inner {
				full_write_required: true,
				old_descriptors: SmallVec::new(),
			}),
			descriptor_capacity: AtomicU32::new(0),
			descriptor: ArcSwapOption::empty(),
			device,
		}
	}

	pub fn alloc_slot<D: DescTypeCpu<ResourceTableCpu = T>>(&self, cpu_type: D::CpuType) -> RCDesc<D> {
		// FIXME: if we run out of capacity in the descriptor array and need to allocate a new one:
		// 	* after this call returns we MUST return the new descriptor set when queried
		// 	* and the old set must still be flushed when we flush, though we can also flush it immediately
		//  * may be better to have one descriptor pool per table instead of a global one, so resources are freed easier
		let slot = self.slots.allocate(D::to_table(cpu_type));
		self.queue.push(slot.clone()).unwrap();
		self.grow_descriptor(slot.id());
		RCDesc::new(slot)
	}

	#[inline]
	fn grow_descriptor(&self, max_id: u32) {
		if max_id < self.descriptor_capacity.load(Relaxed) {
			return;
		}
		self.grow_descriptor_slow(max_id);
	}

	#[cold]
	fn grow_descriptor_slow(&self, max_id: u32) {
		let mut spin_wait = SpinWait::new();
		loop {
			if let Some(inner) = self.inner.try_lock() {
				// invalidate current descriptor, stays invalid until queried for
				// particularly useful during bootup where invalidations are common, but descriptors are rarely used
				if let Some(old) = self.descriptor.swap(None) {
					inner.old_descriptors.push(old);
				}
				// *minimum* guaranteed increase in capacity, actual descriptor allocation may decide to allocate more
				self.descriptor_capacity.store(self.grow_factor(max_id), Release);
				return;
			}
			if max_id < self.descriptor_capacity.load(Relaxed) {
				// another thread already increased capacity
				return;
			}
			spin_wait.spin();
		}
	}

	fn grow_factor(&self, max_id: u32) -> u32 {
		max_id.checked_next_power_of_two().expect("Overflow!")
	}

	// FIXME: Problem: this guarantee is useless with reusable secondary command buffers. You want to record them once and use them over multiple frames, but the DescriptorSet could have changed. This issue may make "large allocation up front" the better way.
	/// a Descriptor gotten from here is guaranteed to contain every allocation that happened before this call, after it has been flushed with [`Self::flush()`].
	#[inline]
	pub fn get_descriptor(&self, lock: AtomicRCSlotsLock<T::SlotType>) -> Arc<DescriptorSet> {
		if let Some(descriptor) = self.descriptor.load_full() {
			descriptor
		} else {
			self.get_descriptor_slow(lock)
		}
	}

	// FIXME lock not needed?
	#[cold]
	pub fn get_descriptor_slow(&self, lock: AtomicRCSlotsLock<T::SlotType>) -> Arc<DescriptorSet> {
		let mut spin_wait = SpinWait::new();
		loop {
			if let Some(inner) = self.inner.try_lock() {
				let count = loop {
					// try overallocating slots if we're close to hitting capacity already
					let slots = self.slots.slots_allocated();
					let count = self.grow_factor(
						u32::try_from(slots as f32 * REALLOCATION_OVERALLOCATION_FACTOR)
							.ok()
							.and_then(|s| s.checked_add(slots))
							.unwrap_or(slots),
					);
					// allows grow_descriptor() spinners to make progress
					let prev = self.descriptor_capacity.fetch_max(count, Relaxed);
					// unlikely, but while we are computing all this, we could hit capacity already, so recompute count
					// if no other thread is making progress, this is guaranteed to be true, as
					// descriptor_capacity <= grow_factor(slots.slots_allocated())
					if prev <= count {
						break count;
					}
				};

				// we write the descriptors once we flush
				let new = DescriptorSet::new_variable(
					self.descriptor_set_allocator.clone(),
					self.descriptor_set_layout.clone(),
					count,
					[],
					[],
				)
				.unwrap();

				// allows get_descriptor_slow() spinners to make progress
				if let Some(old) = self.descriptor.swap(Some(new.clone())) {
					inner.old_descriptors.push(old);
				}
				return new;
			}
			if let Some(descriptor) = self.descriptor.load_full() {
				return descriptor;
			}
			spin_wait.spin();
		}
	}

	pub fn flush(&self, lock: AtomicRCSlotsLock<T::SlotType>) {
		let inner = self.inner.lock();
		if inner.full_write_required {
			self.flush_full(&inner, lock)
		} else {
			self.flush_incremental(&inner)
		}
		inner.full_write_required = false;
	}

	fn flush_full(&self, inner: &crate::sync::MutexGuard<Inner>, lock: AtomicRCSlotsLock<T::SlotType>) {
		let mut writes: SmallVec<[_; 8]> = SmallVec::new();
		{
			let old_descriptors: SmallVec<[_; 1]> = {
				let mut vec = mem::replace(&mut inner.old_descriptors, SmallVec::new())
					.into_iter()
					.map(|d| (d.variable_descriptor_count(), d))
					.collect();
				// unlikely
				if vec.len() > 1 {
					vec.sort_unstable_by_key(|(c, _)| c);
				}
				vec
			};

			{
				let mut old_descriptor_index = 0;
				let mut cutoff = old_descriptors.first().map(|(c, _)| *c).unwrap_or(!0);

				// empty queue, updates scheduled here will be caught by full update below
				while let Some(_) = self.queue.pop() {}

				let mut iter = lock
					.iter_with(|slot| slot.map(|s| s.with(|s| s.clone())))
					// force flush at the end
					.chain([None])
					.enumerate();

				// we allocate worst case right away, reallocating is *probably* way worse than wasting some memory
				let mut buffer = Vec::with_capacity(iter.len());
				while let Some((index, slot)) = iter.next() {
					let flush = if let Some(slot) = slot {
						buffer.push(slot);
						false
					} else {
						true
					};

					let flush_old = index == cutoff;
					if flush || flush_old {
						if !buffer.is_empty() {
							// flush
							writes.push(T::write_descriptor_set(0, 0, buffer.drain(..)));
						}
					}
					if flush_old {
						let (_, old) = old_descriptors.get(old_descriptor_index).unwrap();
						// Safety: update-after-bind does NOT need external sync
						unsafe {
							// TODO that clone is technically unnecessary, if vulkano accepted slices
							old.update_by_ref(writes.clone(), []).unwrap();
						}

						old_descriptor_index += 1;
						cutoff = old_descriptors.get(old_descriptor_index).map(|(c, _)| *c).unwrap_or(!0);
					}
				}
			}
		}
	}

	fn flush_incremental(&self, inner: &crate::sync::MutexGuard<Inner>) {
		assert!(
			inner.old_descriptors.is_empty(),
			"incremental cannot handle old_descriptors!"
		);
		let guard = self.descriptor.load();
		let desc = guard.as_ref().expect("No descriptor present for incremental update!");

		todo!()
	}
}
