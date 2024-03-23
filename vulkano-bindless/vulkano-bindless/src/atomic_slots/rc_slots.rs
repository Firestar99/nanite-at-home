use std::fmt::{Debug, Formatter};
use std::mem::MaybeUninit;
use std::num::Wrapping;
use std::ops::{Deref, Index};
use std::sync::atomic::Ordering;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;

use VersionState::*;

use crate::atomic_slots::atomic_slots::{AtomicSlots, SlotKey};
use crate::atomic_slots::queue::{ChainQueue, PopQueue, QueueSlot};
use crate::atomic_slots::timestamp::Timestamp;
use crate::atomic_slots::Queue;
use crate::sync::atomic::fence;
use crate::sync::atomic::AtomicU32;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
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
			if cfg!(debug_assertions) {
				Slot::<T>::assert_version_state(slot.version.load(Relaxed), Alive);
			}
			f(slot)
		})
	}

	pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
		unsafe { self.with_slot(|slot| slot.t.with(|t| f(t.assume_init_ref()))) }
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
		let _prev = self.with_slot(|slot| slot.atomic.fetch_sub(1, Relaxed));
		debug_assert!(_prev > 0, "Invalid state: Slot is alive but ref count was 0!");
		if _prev == 1 {
			fence(Acquire);
			// SAFETY: we just verified that we are the last RC to be dropped and have exclusive access to this slot's internals
			unsafe {
				self.slots.slot_starts_dying(self);
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
		unsafe { (&*self.slots.inner.index(self.key).t.get()).assume_init_ref() }
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

/// A `Slot` is the backing slot of [`RCSlot`] and stored within [`AtomicSlots`]. The atomic `version` determines the state this slot is in. When switching states
/// `version` will always be wrapping incremented so that each reuse of a slot results in a different version (apart from wrapping around after many reuses).
///
/// # slot is alive: `version & 1 == 1`
/// * `atomic` is the ref count of the alive slot. Should it decrement to 0, the slot will be "start dying" by being added to the
/// [reaper queue](AtomicRCSlots::reaper_queue_add), but may stay alive for some time.
/// * `t` is initialized, may be referenced by many shared references and contains the data contents of this slot.
/// * `free_timestamp` is unused and should not be accessed, as during state transitions a mut reference may be held against it.
/// * Upgrading weak pointers will succeed and increment the ref count.
///
/// # slot is dead: `version & 1 == 0`
/// * `atomic` is *generally* undefined and should not be accessed externally. Typically, it's used to point to the next free slot index while the slot is in any of the
/// queues. However, during transition between the states (e.g. allocation and freeing), `atomic` is undefined.
/// * `t` is uninitialized and should not be accessed, as during state transitions a mut reference may be held against it.
/// * `free_timestamp` is the timestamp until which the slot may be in use, if the `finished_lock_counter` timestamp progresses past this the slot may be dropped and
/// reused.
/// * Upgrading weak pointers will fail.
struct Slot<T> {
	atomic: AtomicU32,
	version: AtomicU32,
	free_timestamp: UnsafeCell<Timestamp>,
	t: UnsafeCell<MaybeUninit<T>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, FromPrimitive, ToPrimitive)]
#[repr(u32)]
enum VersionState {
	/// slot is dead and `t` is uninit
	Dead = 0,
	/// slot is alive, `t` may be accessed through RCSlot<T> or via a held lock
	Alive = 1,
	/// slot's `free_timestamp` has been set and is about to be put into the reaper queue. `t` may only be accessed if the lock's timestamp happened before `free_timestamp`.
	Reaper = 2,
	// Unused = 3,
}

impl VersionState {
	/// the max value a VersionState variant's integer may be, to the next power of 2
	const MAX: u32 = 4;
	const MASK: u32 = Self::MAX - 1;
}

impl VersionState {
	fn from(version: u32) -> Self {
		VersionState::from_u32(version & Self::MASK).unwrap()
	}
}

impl<T> Slot<T> {
	fn version_swap(&self, from: VersionState, to: VersionState, ordering: Ordering) {
		let diff = if to as u32 > from as u32 { 0 } else { VersionState::MAX } + to as u32 - from as u32;
		let old = self.version.fetch_add(diff, ordering);
		Self::assert_version_state(old, from);
	}

	fn assert_version_state(version: u32, expected: VersionState) {
		let state = VersionState::from(version);
		assert_eq!(
			state, expected,
			"Version {} (state: {:?}) differed from expected state {:?}!",
			version, state, expected
		);
	}
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

impl<T> Default for Slot<T> {
	fn default() -> Self {
		Self {
			atomic: AtomicU32::new(0),
			version: AtomicU32::new(0),
			free_timestamp: UnsafeCell::new(Timestamp::new(0)),
			t: UnsafeCell::new(MaybeUninit::uninit()),
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

	pub fn iter_with<'a, R>(
		&'a self,
		f: impl FnMut(Option<&T>) -> R + 'a,
	) -> impl Iterator<Item = R> + ExactSizeIterator + 'a {
		self.slots.iter_with(self.lock_timestamp, f)
	}
}

impl<T> Drop for AtomicRCSlotsLock<T> {
	fn drop(&mut self) {
		self.slots.unlock(self.lock_timestamp);
	}
}

pub struct AtomicRCSlots<T> {
	inner: AtomicSlots<Slot<T>>,
	slots_allocated_max: AtomicU32,
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
	_unimpl_send_sync: UnsafeCell<()>,
}

impl<T> AtomicRCSlots<T> {
	pub fn new(first_block_size: u32) -> Arc<Self> {
		let inner = AtomicSlots::new(first_block_size);
		Arc::new(Self {
			reaper_queue: ChainQueue::new(&inner),
			dead_queue: PopQueue::new(&inner),
			inner,
			slots_allocated_max: AtomicU32::new(0),
			curr_lock_counter: AtomicU32::new(0),
			finished_lock_counter: AtomicU32::new(0),
			_unimpl_send_sync: UnsafeCell::new(()),
			// finished_locks_bits: AtomicU64::new(!0),
		})
	}

	pub fn allocate(self: &Arc<Self>, t: T) -> RCSlot<T> {
		let (key, new_alloc) = match self.dead_queue.pop(&self.inner) {
			None => (self.inner.allocate(), true),
			Some(e) => (e, false),
		};

		self.inner.with(key, |slot| {
			// Safety: we are the only ones who have access to this newly allocated key
			unsafe {
				slot.t.with_mut(|t_ref| {
					t_ref.write(t);
				});
			}
			// ref count of 1
			slot.atomic.store(1, Relaxed);
			// slot is now alive and t may be referenced by many shared references beyond this point
			slot.version_swap(Dead, Alive, Release);
		});

		if new_alloc {
			// Relaxed may be enough, but I want to make sure the slot is properly initialized before iterated on
			self.slots_allocated_max
				.fetch_max(self.inner.key_to_raw_index(key) + 1, Release);
		}

		// Safety: transfer ownership of the ref increment done above
		unsafe { RCSlot::new(self.clone(), key) }
	}

	/// # Safety
	/// must only be called when the last RC was dropped, so we can acquire exclusive ownership
	#[cold]
	#[inline(never)]
	unsafe fn slot_starts_dying(&self, key: &RCSlot<T>) {
		// FIXME may race against locking, free_now may be set to true even though another thread just locked
		let curr = Timestamp::new(self.curr_lock_counter.load(Relaxed));
		let finished = Timestamp::new(self.finished_lock_counter.load(Relaxed));
		// free_now means there are no locks present
		let free_now = curr.compare_wrapping(&finished).unwrap().is_le();

		if free_now {
			key.with_slot(|slot| {
				// FIXME freeing may race against locked iterating
				Self::free_slot(slot, Alive);
			});
			// put onto dead queue
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.dead_queue.push(&self.inner, key.key);
		} else {
			key.with_slot(|slot| {
				// SAFETY: while the slot is (still) alive, no-one is allowed to hold a reference against free_timestamp, thus grabbing a mutable ref is safe.
				// Also, the method contract ensures only one thread may enter this method for each alive slot.
				unsafe {
					slot.free_timestamp.with_mut(|free_timestamp| *free_timestamp = curr);
				}

				// free_timestamp was set and may now be read by others, like lock iteration
				slot.version_swap(Alive, Reaper, Release);
			});

			// put onto reaper queue
			// the order they are put into the queue may differ from their timestamps, so a few entries may get stuck
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.reaper_queue.push(&self.inner, key.key);
		}
	}

	/// Frees the alive slot and turns it dead. Panics if the slot was not alive.
	///
	/// # Safety
	/// Must have exclusive access to `slot.t`
	unsafe fn free_slot(slot: &Slot<T>, from: VersionState) {
		slot.version_swap(from, Dead, Relaxed);
		// nobody should be accessing t, so ordering of version_swap does not matter
		unsafe {
			slot.t.with_mut(|t| {
				t.assume_init_drop();
			});
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
		let chain = self.reaper_queue.pop_chain(&self.inner, |key| {
			// SAFETY: we have exclusive access to popped entries, and to drop their t now that it's safe to do so
			unsafe {
				self.inner.with(key, |slot| {
					// TODO move this to docs
					// It is required to be less_than and not just equal, as some entries may get stuck due to:
					// * reaper_queue internally retaining a single entry
					// * entries being added out of order compared to their timestamp
					// Thus we also have to free previously unlocked entries, which have gone stuck.
					// But there is the risk: Constant locking without any new entries to flush the queue can cause timestamps to wrap around,
					// and then we don't know if it was before or after us!
					let free_timestamp = slot.free_timestamp.with(|t| *t);
					if free_timestamp
						.compare_wrapping(&lock_timestamp)
						.expect("Reaper queue stood still for too long, timestamps have wrapped around!")
						.is_le()
					{
						// Safety: has exclusive access to slot, as `ChainQueue` poping is mutexed
						Self::free_slot(slot, Reaper);
						true
					} else {
						false
					}
				})
			}
		});

		// unlock complete
		self.finished_lock_counter.store(lock_timestamp.into(), Release);

		// move freed slots to dead queue
		// freed slots may reorder here, that's fine.
		if let Some(chain) = chain {
			self.dead_queue.push_chain(&self.inner, chain);
		}
	}

	/// Iterates over all slots that have been allocated thus far.
	/// Does not give [`RCSlot`] but plain `&T` to prevent resurrection of slots in reaper state. Could be implemented later if needed.
	fn iter_with<'a, R>(
		self: &'a Arc<Self>,
		lock_timestamp: Timestamp,
		mut f: impl FnMut(Option<&T>) -> R + 'a,
	) -> impl Iterator<Item = R> + ExactSizeIterator + 'a {
		let max = self.slots_allocated_max.load(Relaxed);

		// it may be possible to iterate more efficiently on a per-block basis?
		// and at the same time ensure ExactSizeIterator?
		(0..max).map(move |index| {
			// Safety: it is a valid index
			let key = unsafe { self.inner.key_from_raw_index(index) };
			self.inner.with(key, |slot| {
				let present = match VersionState::from(slot.version.load(Relaxed)) {
					Dead => false,
					Alive => true,
					Reaper => {
						// Safety: Reaper state ensures free_timestamp has been written
						let free_timestamp = unsafe { slot.free_timestamp.with(|t| *t) };
						free_timestamp.compare_wrapping(&lock_timestamp).unwrap().is_ge()
					}
				};

				if present {
					// Safety: just checked that we can safely access it
					unsafe { slot.t.with(|t| f(Some(t.assume_init_ref()))) }
				} else {
					f(None)
				}
			})
		})
	}
}

impl<T> Drop for AtomicRCSlots<T> {
	fn drop(&mut self) {
		// Need to drop all slots remaining in the reaper queue (always either 0 or the 1 retained slot)
		// Safety: we have exclusive access of all of our slots
		unsafe {
			self.reaper_queue.dry_up(&self.inner, |key| {
				self.inner.with(key, |slot| {
					Self::free_slot(slot, Reaper);
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
	#[should_panic(expected = "(state: Dead) differed from expected state Alive!")]
	fn test_ref_counting_underflow() {
		let slots = AtomicRCSlots::new(32);
		let slot = slots.allocate(42);
		
		// Safety: this is not safe
		unsafe { slot.ref_dec() };
		drop(slot);
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

	#[test]
	fn test_lock_timestamp_ordering() {
		let slots = AtomicRCSlots::new(32);

		let arc = Arc::new(42);
		let slot = slots.allocate(arc.clone());
		let slot_flush = slots.allocate(Arc::new(42));
		assert_eq!(Arc::strong_count(&arc), 2);

		let lock1 = slots.lock();
		let lock2 = slots.lock();
		assert_eq!(Arc::strong_count(&arc), 2);

		drop(slot);
		drop(slot_flush);
		assert_eq!(Arc::strong_count(&arc), 2);

		lock1.unlock();
		assert_eq!(Arc::strong_count(&arc), 2);

		lock2.unlock();
		assert_eq!(Arc::strong_count(&arc), 1);
	}

	fn iter_collect<T: Clone>(lock: &AtomicRCSlotsLock<T>) -> Vec<Option<T>> {
		lock.iter_with(|t| t.cloned()).collect::<Vec<_>>()
	}

	#[test]
	fn test_iter_smoke() {
		let slots = AtomicRCSlots::new(32);
		assert_eq!(iter_collect(&slots.lock()), []);

		let slot1 = slots.allocate(42);
		assert_eq!(iter_collect(&slots.lock()), [Some(42)]);

		let slot2 = slots.allocate(69);
		assert_eq!(iter_collect(&slots.lock()), [Some(42), Some(69)]);

		drop(slot2);
		assert_eq!(iter_collect(&slots.lock()), [Some(42), None]);

		drop(slot1);
		assert_eq!(iter_collect(&slots.lock()), [None, None]);
	}

	#[test]
	fn test_iter_locked() {
		let slots = AtomicRCSlots::new(32);
		assert_eq!(iter_collect(&slots.lock()), []);

		let slot1 = slots.allocate(1);
		let slot2 = slots.allocate(2);
		let slot3 = slots.allocate(3);
		assert_eq!(iter_collect(&slots.lock()), [Some(1), Some(2), Some(3)]);

		// 1 lock
		let lock1 = slots.lock();
		assert_eq!(iter_collect(&lock1), [Some(1), Some(2), Some(3)]);
		drop(slot1);
		assert_eq!(iter_collect(&lock1), [Some(1), Some(2), Some(3)]);
		drop(lock1);
		assert_eq!(iter_collect(&slots.lock()), [None, Some(2), Some(3)]);

		// 2 locks in parallel
		let lock2 = slots.lock();
		assert_eq!(iter_collect(&lock2), [None, Some(2), Some(3)]);
		drop(slot2);
		assert_eq!(iter_collect(&lock2), [None, Some(2), Some(3)]);

		let lock3 = slots.lock();
		assert_eq!(iter_collect(&lock2), [None, Some(2), Some(3)]);
		assert_eq!(iter_collect(&lock3), [None, None, Some(3)]);
		drop(slot3);
		assert_eq!(iter_collect(&lock2), [None, Some(2), Some(3)]);
		assert_eq!(iter_collect(&lock3), [None, None, Some(3)]);

		drop(lock2);
		assert_eq!(iter_collect(&lock3), [None, None, Some(3)]);

		drop(lock3);
		assert_eq!(iter_collect(&slots.lock()), [None, None, None]);
	}
}
