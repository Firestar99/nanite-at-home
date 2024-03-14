use std::fmt::{Debug, Formatter};
use std::mem::MaybeUninit;
use std::num::Wrapping;
use std::ops::{Deref, Index};

use crate::atomic_slots::atomic_slots::{AtomicSlots, SlotKey};
use crate::atomic_slots::queue::{ChainQueue, PopQueue, QueueSlot};
use crate::atomic_slots::timestamp::Timestamp;
use crate::atomic_slots::Queue;
use crate::sync::atomic::fence;
use crate::sync::atomic::AtomicU32;
use crate::sync::atomic::Ordering::{Acquire, Relaxed};
use crate::sync::cell::UnsafeCell;
use crate::sync::{Arc, SpinWait};

pub struct RCSlot<T> {
	// TODO get rid of that Arc
	slots: Arc<AtomicRCSlots<T>>,
	key: SlotKey,
}

impl<T> RCSlot<T> {
	/// Creates a new RCSlot
	///
	/// # Safety
	/// ref_count must have been incremented for this slot previously, and ownership to decrement it again is transferred to Self
	#[inline]
	unsafe fn new(slots: Arc<AtomicRCSlots<T>>, key: SlotKey) -> Self {
		Self { slots, key }
	}

	fn with_slot<R>(&self, f: impl FnOnce(&Slot<T>) -> R) -> R {
		self.slots.inner.with(self.key, |slot| {
			// SAFETY: we must assume the slot to be alive, to check if it really is alive. As when it is alive, reading version with a shared ref is ok.
			let _version = unsafe { slot.inner.with(|inner| (&*inner).version) };
			debug_assert_eq!(_version.0 & 1, 1, "Slot is alive with version {}", _version);
			f(slot)
		})
	}

	pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
		unsafe { self.with_slot(|slot| slot.inner.with(|inner| f(inner.t.assume_init_ref()))) }
	}

	/// # Safety
	/// must follow ref counting: after incrementing one must also decrement exactly that many times
	#[inline]
	pub unsafe fn ref_inc(&self) {
		let _prev = self.with_slot(|slot| slot.atomic.fetch_add(1, Relaxed));
		debug_assert!(_prev > 0, "Invalid state: Slot is alive but ref count was 0!");
	}

	/// # Safety
	/// must follow ref counting: after incrementing one must also decrement exactly that many times
	#[inline]
	pub unsafe fn ref_dec(&self) {
		let prev = self.with_slot(|slot| slot.atomic.fetch_sub(1, Relaxed));
		debug_assert!(prev > 0, "Invalid state: Slot is alive but ref count was 0!");
		if prev == 1 {
			fence(Acquire);
			// SAFETY: we just verified that we are the last RC to be dropped
			unsafe {
				self.slots.reaper_queue_add(self);
			}
		}
	}

	pub fn ref_count(&self) -> u32 {
		self.with_slot(|slot| slot.atomic.load(Relaxed))
	}
}

/// loom cannot reason with references
#[cfg(not(feature = "loom_tests"))]
impl<T> Deref for RCSlot<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// Safety:
		unsafe { (&*self.slots.inner.index(self.key).inner.get()).t.assume_init_ref() }
	}
}

impl<T: Copy> RCSlot<T> {
	/// replacement for deref if loom is in use
	pub fn deref_copy(&self) -> T {
		self.with(|t| *t)
	}
}

impl<T> Clone for RCSlot<T> {
	fn clone(&self) -> Self {
		// SAFETY: we are ref counting
		unsafe {
			self.ref_inc();
			Self::new(self.slots.clone(), self.key)
		}
	}
}

impl<T> Drop for RCSlot<T> {
	fn drop(&mut self) {
		// SAFETY: we are ref counting
		unsafe {
			self.ref_dec();
		}
	}
}

impl<T> Debug for RCSlot<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RCSlot").field("ref_count", &self.ref_count()).finish()
	}
}

/// An `RCSlotInner` is the backing slot in [`AtomicSlots`] that manages its lifetime via RC. `version` determines the state this slot is in:
///
/// # slot is alive: `version & 1 == 1`
/// `atomic` is the ref count of the alive slot. `t` is initialized and contains the data contents of this slot. `free_timestamp` is unused. Upgrading weak pointers will
/// succeed and increment the ref count.
///
/// # slot is dead: `version & 1 == 0`
/// `atomic` points to the next free slot index in this [`AtomicSlots`], as controlled by [`PopQueue`] or [`ChainQueue`]. `t` is uninitialized. `free_timestamp` is the
/// timestamp until which the slot may be in use, if the timestamp progresses past this the slot may be dropped and reused. Upgrading weak pointers will fail.
pub struct Slot<T> {
	pub atomic: AtomicU32,
	pub inner: UnsafeCell<SlotInner<T>>,
}

impl<T> QueueSlot for Slot<T> {
	fn atomic(&self) -> &AtomicU32 {
		&self.atomic
	}
}

/// we can't do much more, as we'd need to assume read-only access to inner
impl<T> Debug for Slot<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Slot").field("atomic", &self.atomic).finish()
	}
}

pub struct SlotInner<T> {
	// TODO u32 or is u16 enough?
	pub version: Wrapping<u32>,
	pub free_timestamp: Timestamp,
	/// ensured to be init if `version & 1 == 1`
	/// may be init or uninit if `version & 1 == 0`, determined by whether it is in the reaper queue or not.
	pub t: MaybeUninit<T>,
}

impl<T> Default for Slot<T> {
	fn default() -> Self {
		Self {
			atomic: AtomicU32::new(0),
			inner: UnsafeCell::new(SlotInner {
				version: Wrapping(0),
				free_timestamp: Timestamp::new(0),
				t: MaybeUninit::uninit(),
			}),
		}
	}
}

pub struct AtomicRCSlotsLock<T> {
	// TODO get rid of that Arc
	slots: Arc<AtomicRCSlots<T>>,
	lock_timestamp: Timestamp,
}

impl<T> AtomicRCSlotsLock<T> {
	pub fn unlock(self) {
		// impl in drop
	}
}

impl<T> Drop for AtomicRCSlotsLock<T> {
	fn drop(&mut self) {
		self.slots.unlock(self.lock_timestamp);
	}
}

pub struct AtomicRCSlots<T> {
	inner: AtomicSlots<Slot<T>>,
	/// queue of slots that can be dropped and reclaimed, as soon as all locks that may be using them have finished
	reaper_queue: ChainQueue<Slot<T>>,
	/// queue of slots that are dead and may be reused
	dead_queue: PopQueue<Slot<T>>,
	/// Every created lock gets a new timestamp from this counter. Current active locks is `next_lock_counter.wrapping_sub(finished_lock_counter)`. May wrap around.
	curr_lock_counter: AtomicU32,
	/// Counter to track the amount of locks that have been unlocked. Current active locks is `next_lock_counter.wrapping_sub(finished_lock_counter)`. May wrap around.
	finished_lock_counter: AtomicU32,
	// /// A bitset of the next 64 locks that are set to true once they unlock. Allow locks to be freed in arbitrary order without blocking on each other,
	// /// as long as it's only 64 locks ahead of the oldest locked one.
	// finished_locks_bits: AtomicU64,
}

impl<T> AtomicRCSlots<T> {
	pub fn new(first_block_size: u32) -> Arc<Self> {
		let inner = AtomicSlots::new(first_block_size);
		Arc::new(Self {
			reaper_queue: ChainQueue::new(&inner),
			dead_queue: PopQueue::new(&inner),
			inner,
			curr_lock_counter: AtomicU32::new(0),
			finished_lock_counter: AtomicU32::new(0),
			// finished_locks_bits: AtomicU64::new(!0),
		})
	}

	pub fn allocate(self: &Arc<Self>, t: T) -> RCSlot<T> {
		let key = self
			.dead_queue
			.pop(&self.inner)
			.unwrap_or_else(|| self.inner.allocate());

		self.inner.with(key, |slot| {
			// Safety: we are the only ones who have access to this newly allocated key
			unsafe {
				slot.inner.with_mut(|inner| {
					assert_eq!(inner.version.0 & 1, 0, "slot that was allocated was not dead");
					inner.version += 1;
					inner.t.write(t);
				});
			}
			slot.atomic.store(1, Relaxed);
		});

		// Safety: transfer ownership of the ref increment done above
		unsafe { RCSlot::new(self.clone(), key) }
	}

	/// # Safety
	/// must only be called when the last RC was dropped, and we can acquire exclusive ownership
	#[cold]
	#[inline(never)]
	unsafe fn reaper_queue_add(&self, key: &RCSlot<T>) {
		let curr = Timestamp::new(self.curr_lock_counter.load(Relaxed));
		let finished = Timestamp::new(self.finished_lock_counter.load(Relaxed));
		// free_now means there are no locks present
		let free_now = curr.compare_wrapping(&finished).unwrap().is_le();

		// SAFETY: see method safely contract
		unsafe {
			key.with_slot(|slot| {
				slot.inner.with_mut(|inner| {
					assert_eq!(inner.version.0 & 1, 1, "slot is alive before it was freed");
					inner.version += 1;
					inner.free_timestamp = curr;
					if free_now {
						inner.t.assume_init_drop();
					}
				})
			});
		}

		if free_now {
			// put onto dead queue
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.dead_queue.push(&self.inner, key.key);
		} else {
			// put onto reaper queue
			// the order they are put into the queue may differ from their timestamps, so a few entries may get stuck
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.reaper_queue.push(&self.inner, key.key);
		}
	}

	/// # Safety
	/// Must only be called from unlock
	unsafe fn reaper_queue_free(&self, lock_timestamp: Timestamp) {
		let chain = self.reaper_queue.pop_chain(&self.inner, |key| {
			// SAFETY: we have exclusive access to popped entries, and to drop their t now that it's safe to do so
			unsafe {
				self.inner.with(key, |slot| {
					slot.inner.with_mut(|inner| {
						// TODO move this to docs
						// It is required to be less_than and not just equal, as some entries may get stuck due to:
						// * reaper_queue internally retaining a single entry
						// * entries being added out of order compared to their timestamp
						// Thus we also have to free previously unlocked entries, which have gone stuck.
						// But there is the risk that constant locking without any new entries to flush the queue can cause timestamps to wrap around,
						// and then we don't know if it was before or after us!
						if inner
							.free_timestamp
							.compare_wrapping(&lock_timestamp)
							.expect("Reaper queue stood still for too long, timestamps have wrapped around!")
							.is_le()
						{
							inner.t.assume_init_drop();
							true
						} else {
							false
						}
					})
				})
			}
		});

		// freed slots may reorder here, that's fine.
		// But actually, won't happen with the current unlock logic.
		if let Some(chain) = chain {
			self.dead_queue.push_chain(&self.inner, chain);
		}
	}

	pub fn lock(self: &Arc<Self>) -> AtomicRCSlotsLock<T> {
		let lock_id = Timestamp(Wrapping(self.curr_lock_counter.fetch_add(1, Relaxed)) + Wrapping(1));
		AtomicRCSlotsLock {
			slots: self.clone(),
			lock_timestamp: lock_id,
		}
	}

	fn unlock(self: &Arc<Self>, lock_timestamp: Timestamp) {
		let mut spin_wait = SpinWait::new();

		// wait for all previous locks to unlock
		// TODO impl finished_locks_bits logic
		loop {
			let old = Wrapping(self.finished_lock_counter.load(Relaxed));
			if old == lock_timestamp.0 - Wrapping(1) {
				break;
			}
			spin_wait.spin();
		}

		// free reaper_queue
		// SAFETY: I am unlock
		unsafe {
			self.reaper_queue_free(lock_timestamp);
		}

		self.finished_lock_counter.store(lock_timestamp.into(), Relaxed);
	}
}

impl<T> Drop for AtomicRCSlots<T> {
	fn drop(&mut self) {
		// Safety: we have exclusive ownership of ourselves and our slots, and need to drop all slots in the reaper queue
		unsafe {
			self.reaper_queue.dry_up(&self.inner, |key| {
				self.inner.with(key, |slot| {
					slot.inner.with_mut(|inner| {
						inner.t.assume_init_drop();
					})
				})
			});
		}
	}
}

#[cfg(test)]
mod test_utils {
	use std::mem::replace;

	use super::*;

	pub struct LockUnlock<T> {
		slots: Arc<AtomicRCSlots<T>>,
		lock: AtomicRCSlotsLock<T>,
	}

	impl<T> LockUnlock<T> {
		pub fn new(slots: &Arc<AtomicRCSlots<T>>) -> Self {
			Self {
				slots: slots.clone(),
				lock: slots.lock(),
			}
		}

		pub fn advance(&mut self) {
			replace(&mut self.lock, self.slots.lock()).unlock();
		}
	}
}

#[cfg(all(test, not(feature = "loom_tests")))]
mod tests {
	use crate::atomic_slots::rc_slots::test_utils::LockUnlock;

	use super::*;

	#[test]
	fn test_ref_counting() {
		let slots = AtomicRCSlots::new(32);
		let slot = slots.allocate(42);
		assert_eq!(slot.deref_copy(), 42);
		assert_eq!(slot.ref_count(), 1);

		{
			let slotc = slot.clone();
			assert_eq!(slotc.deref_copy(), 42);
			assert_eq!(slot.deref_copy(), 42);
			assert_eq!(slotc.ref_count(), 2);
			assert_eq!(slot.ref_count(), 2);
		}

		assert_eq!(slot.deref_copy(), 42);
		assert_eq!(slot.ref_count(), 1);
	}

	#[test]
	fn test_alloc_unique() {
		let slots = AtomicRCSlots::new(32);

		let count: u32 = 5;
		let vec = (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();
		for (i, slot) in vec.iter().enumerate() {
			assert_eq!(slot.deref_copy(), i as u32);
			assert_eq!(slots.inner.key_to_raw_index(slot.key), i as u32);

			assert_eq!(slot.ref_count(), 1);
			{
				let slot = slot.clone();
				assert_eq!(slot.ref_count(), 2);
			}
			assert_eq!(slot.ref_count(), 1);
		}
	}

	#[test]
	fn test_queues() {
		unsafe {
			let slots = AtomicRCSlots::new(32);
			let mut lock_unlock = LockUnlock::new(&slots);

			let arc1 = Arc::new(42);
			let slot1 = slots.allocate(arc1.clone());
			assert_eq!(slots.inner.key_to_raw_index(slot1.key), 0);
			let arc2 = Arc::new(69);
			let slot2 = slots.allocate(arc2.clone());
			assert_eq!(slots.inner.key_to_raw_index(slot2.key), 1);

			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // alive
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			drop(slot1);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // alive
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // reaper
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			drop(slot2);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 2);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // reaper
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			assert_eq!(Arc::strong_count(&arc1), 1); // dead
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper

			// we asserted that it was freed, useless now
			drop(arc1);

			// new slot will allocate a new slot, not reuse, as dead_queue retains 1 slot
			let arc3 = Arc::new(3);
			let slot3 = slots.allocate(arc3.clone());
			assert_eq!(slots.inner.key_to_raw_index(slot3.key), 2);

			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper
			assert_eq!(Arc::strong_count(&arc3), 2); // alive

			drop(slot3);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 2);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 2);
			assert_eq!(Arc::strong_count(&arc2), 1); // dead
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper

			// we asserted that it was freed, useless now
			drop(arc2);

			// new slot will reuse slot 1
			let arc4 = Arc::new(4);
			let slot4 = slots.allocate(arc4.clone());
			assert_eq!(slots.inner.key_to_raw_index(slot4.key), 0);

			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper
			assert_eq!(Arc::strong_count(&arc4), 2); // alive

			// new slot will allocate a new slot, not reuse, as dead_queue retains 1 slot
			let arc5 = Arc::new(5);
			let slot5 = slots.allocate(arc5.clone());
			assert_eq!(slots.inner.key_to_raw_index(slot5.key), 3);

			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper
			assert_eq!(Arc::strong_count(&arc4), 2); // alive
			assert_eq!(Arc::strong_count(&arc5), 2); // alive
		}
	}

	// TODO test slot version!
	#[test]
	fn test_queues_many_entries() {
		unsafe {
			let slots = AtomicRCSlots::new(32);
			let mut lock_unlock = LockUnlock::new(&slots);

			for i in 0..5 {
				let slot = slots.allocate(());
				assert_eq!(slots.inner.key_to_raw_index(slot.key), i);
			}
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 5);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);

			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 4);

			// 3 reused
			for i in 0..3 {
				let slot = slots.allocate(());
				assert_eq!(slots.inner.key_to_raw_index(slot.key), i);
				assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1 + i);
				assert_eq!(slots.dead_queue.debug_count(&slots.inner), 3 - i);
			}

			// 2 newly allocated
			for i in 0..2 {
				let slot = slots.allocate(());
				assert_eq!(slots.inner.key_to_raw_index(slot.key), i + 5);
				assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 4 + i);
				assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);
			}

			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 6);
		}
	}

	#[test]
	fn test_queues_while_unlocked() {
		unsafe {
			let slots = AtomicRCSlots::new(32);

			drop(slots.allocate(0));
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			let slot = slots.allocate(1);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			drop(slot);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 2);

			let slot = slots.allocate(2);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			drop(slot);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 2);
		}
	}

	#[test]
	fn test_queues_mix_locked_and_unlocked() {
		unsafe {
			let slots = AtomicRCSlots::new(32);
			let alloc = |count: u32| (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();

			// unlocked behaviour
			// 5 new
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 5);

			// locked behaviour
			let lock = slots.lock();
			// 1 new, 4 reused
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 5);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			lock.unlock();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 5);

			// unlocked behaviour
			// 1 new, 4 reused
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 1);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 6);
		}
	}

	#[test]
	fn test_queues_drop_before_and_after_lock() {
		unsafe {
			let slots = AtomicRCSlots::new(32);
			let alloc = |count: u32| (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();

			let before_lock_a = alloc(2);
			let before_lock_b = alloc(3);
			let lock = slots.lock();
			let after_lock_a = alloc(4);
			let after_lock_b = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);

			drop(before_lock_a);
			drop(after_lock_a);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 6);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 0);

			lock.unlock();
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 5);

			drop(before_lock_b);
			drop(after_lock_b);
			assert_eq!(slots.reaper_queue.debug_count(&slots.inner), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.inner), 13);
		}
	}

	#[test]
	fn test_reaper_queue_leak() {
		let slots = AtomicRCSlots::new(32);

		let arc = Arc::new(42);
		let slot = slots.allocate(arc.clone());
		assert_eq!(Arc::strong_count(&arc), 2);

		// get slot in the reaper queue, where it remains unfreed
		let lock = slots.lock();
		drop(slot);
		lock.unlock();
		assert_eq!(Arc::strong_count(&arc), 2);

		// must free slot in reaper queue
		drop(slots);
		assert_eq!(Arc::strong_count(&arc), 1, "reaper queue is leaking!");
	}
}
