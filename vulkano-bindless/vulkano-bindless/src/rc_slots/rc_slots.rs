use crate::rc_slots::timestamp::Timestamp;
use crate::sync::atomic::fence;
use crate::sync::atomic::AtomicU32;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::cell::UnsafeCell;
use crate::sync::Arc;
use crossbeam_queue::SegQueue;
use crossbeam_utils::CachePadded;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;
use rangemap::RangeSet;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::num::Wrapping;
use std::ops::{Deref, Index};
use std::sync::atomic::{AtomicUsize, Ordering};
use VersionState::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotIndex(pub usize);

impl Deref for SlotIndex {
	type Target = usize;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> Index<SlotIndex> for [Slot<T>] {
	type Output = Slot<T>;

	fn index(&self, index: SlotIndex) -> &Self::Output {
		&self[index.0]
	}
}

pub trait RCSlotsInterface<T> {
	fn drop_slot(&self, index: SlotIndex, t: T);
}

pub struct DefaultRCSlotInterface;

impl<T> RCSlotsInterface<T> for DefaultRCSlotInterface {
	fn drop_slot(&self, _index: SlotIndex, t: T) {
		drop(t);
	}
}

pub struct RCSlot<T, Interface: RCSlotsInterface<T> = DefaultRCSlotInterface> {
	slots: *const RCSlots<T, Interface>,
	index: SlotIndex,
}

unsafe impl<T, Interface: RCSlotsInterface<T>> Send for RCSlot<T, Interface> {}
unsafe impl<T, Interface: RCSlotsInterface<T>> Sync for RCSlot<T, Interface> {}

impl<T, Interface: RCSlotsInterface<T>> RCSlot<T, Interface> {
	/// Creates a new RCSlot of an alive slot
	///
	/// # Safety
	/// the slot must be alive, to ensure the Arc of `slots` is ref counted correctly
	/// `ref_count` must have been incremented for this slot previously, and ownership to decrement it again is transferred to Self
	#[inline]
	unsafe fn new(slots: *const RCSlots<T, Interface>, index: SlotIndex) -> Self {
		Self { slots, index }
	}

	#[inline]
	pub fn slots(&self) -> &RCSlots<T, Interface> {
		unsafe { &*self.slots }
	}

	fn with_slot<R>(&self, f: impl FnOnce(&Slot<T>) -> R) -> R {
		let slot = &self.slots().array[self.index];
		if cfg!(debug_assertions) {
			Slot::<T>::assert_version_state(slot.version.load(Relaxed), Alive);
		}
		f(slot)
	}

	pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
		unsafe { self.with_slot(|slot| slot.t.with(|t| f(t.assume_init_ref()))) }
	}

	/// # Safety
	/// must follow ref counting: after incrementing one must also decrement exactly that many times
	#[inline]
	unsafe fn ref_inc(&self) {
		let _prev = self.with_slot(|slot| slot.atomic.fetch_add(1, Relaxed));
		debug_assert!(_prev > 0, "Invalid state: Slot is alive but ref count was 0!");
	}

	/// Decrements the ref_count, returns true if this was the last ref_dec and the slot started to die.
	///
	/// # Safety
	/// must follow ref counting: after incrementing one must also decrement exactly that many times
	#[inline]
	unsafe fn ref_dec(&self) -> bool {
		let _prev = self.with_slot(|slot| slot.atomic.fetch_sub(1, Relaxed));
		debug_assert!(_prev > 0, "Invalid state: Slot is alive but ref count was 0!");
		if _prev == 1 {
			fence(Acquire);
			// SAFETY: we just verified that we are the last RC to be dropped and have exclusive access to this slot's internals
			unsafe {
				self.slots().slot_starts_dying(self.index);
			}
			true
		} else {
			false
		}
	}

	#[inline]
	pub fn ref_count(&self) -> u32 {
		self.with_slot(|slot| slot.atomic.load(Relaxed))
	}

	#[inline]
	pub fn id(&self) -> SlotIndex {
		self.index
	}

	#[inline]
	pub fn version(&self) -> u32 {
		VersionState::from(self.with_slot(|slot| slot.version.load(Relaxed))).1
	}

	/// turns this clone of `RCSlot` into a [`SlotIndex`]
	///
	/// # Safety
	/// The [`SlotIndex`] returned must be turned back into an `RCSlot` using [`Self::from_raw_index`] eventually, to
	/// ensure sure no resources are leaking
	#[inline]
	pub unsafe fn into_raw_index(self) -> SlotIndex {
		let index = self.index;
		mem::forget(self);
		index
	}

	/// turns a [`SlotIndex`] acquired from [`Self::into_raw_index`] back into an `RCSlot`
	///
	/// # Safety
	/// The [`SlotIndex`] must have originated from [`Self::from_raw_index`], this method must only be called once with
	/// that particular [`SlotIndex`], `slots` must be the same instance as the original `RCSlot` and the T generic must be the same
	#[inline]
	pub unsafe fn from_raw_index(slots: &Arc<RCSlots<T, Interface>>, index: SlotIndex) -> Self {
		RCSlot::new(Arc::as_ptr(slots) as *const _, index)
	}
}

/// loom cannot reason with references
#[cfg(not(feature = "loom_tests"))]
impl<T, Interface: RCSlotsInterface<T>> Deref for RCSlot<T, Interface> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// Safety: self existing ensures the slot must be alive and t exists
		unsafe { (*self.slots().array.index(self.index).t.get()).assume_init_ref() }
	}
}

impl<T: Copy, Interface: RCSlotsInterface<T>> RCSlot<T, Interface> {
	/// replacement for deref if loom is in use
	pub fn deref_copy(&self) -> T {
		self.with(|t| *t)
	}
}

impl<T, Interface: RCSlotsInterface<T>> Clone for RCSlot<T, Interface> {
	fn clone(&self) -> Self {
		// SAFETY: we are ref counting
		unsafe {
			self.ref_inc();
			Self::new(self.slots, self.index)
		}
	}
}

impl<T, Interface: RCSlotsInterface<T>> Drop for RCSlot<T, Interface> {
	fn drop(&mut self) {
		// SAFETY: we are ref counting
		unsafe {
			if self.ref_dec() {
				drop(Arc::from_raw(self.slots));
			}
		}
	}
}

impl<T, Interface: RCSlotsInterface<T>> Debug for RCSlot<T, Interface> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RCSlot").field("ref_count", &self.ref_count()).finish()
	}
}

impl<T, Interface: RCSlotsInterface<T>> PartialEq<Self> for RCSlot<T, Interface> {
	fn eq(&self, other: &Self) -> bool {
		self.slots == other.slots && self.index == other.index
	}
}

impl<T, Interface: RCSlotsInterface<T>> Eq for RCSlot<T, Interface> {}

impl<T, Interface: RCSlotsInterface<T>> Hash for RCSlot<T, Interface> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.slots.hash(state);
		self.index.hash(state);
	}
}

/// A `Slot` is the backing slot of [`RCSlot`] and stored within [`AtomicSlots`]. The atomic `version` determines the state this slot is in. When switching states
/// `version` will always be wrapping incremented so that each reuse of a slot results in a different version (apart from wrapping around after many reuses).
///
/// # slot is alive: `version & 1 == 1`
/// * `atomic` is the ref count of the alive slot. Should it decrement to 0, the slot will be "start dying" by being added to the
/// [reaper queue](RCSlots::reaper_queue_add), but may stay alive for some time.
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

unsafe impl<T> Send for Slot<T> {}
unsafe impl<T> Sync for Slot<T> {}

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
	#[inline]
	fn from(version: u32) -> (Self, u32) {
		let state = VersionState::from_u32(version & Self::MASK).unwrap();
		let version = version & !Self::MASK;
		(state, version)
	}
}

impl<T> Slot<T> {
	fn version_swap(&self, from: VersionState, to: VersionState, ordering: Ordering) {
		let diff = if to as u32 > from as u32 { 0 } else { VersionState::MAX } + to as u32 - from as u32;
		let old = self.version.fetch_add(diff, ordering);
		Self::assert_version_state(old, from);
	}

	fn assert_version_state(version: u32, expected: VersionState) {
		let state = VersionState::from(version).0;
		assert_eq!(
			state, expected,
			"Version {} (state: {:?}) differed from expected state {:?}!",
			version, state, expected
		);
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

pub struct Lock<T, Interface: RCSlotsInterface<T> = DefaultRCSlotInterface> {
	slots: Arc<RCSlots<T, Interface>>,
	lock_timestamp: Timestamp,
}

impl<T, Interface: RCSlotsInterface<T>> Lock<T, Interface> {
	pub fn unlock(self) {
		// impl in drop
	}

	/// Iterates over all slots that have been allocated thus far.
	/// It is NOT sound to clone returned [`RCSlot`] and doing so may result in a panic! Doing so could revive a slot marked to be reaper'ed, which is currently not
	/// (yet) supported.
	pub fn iter_with<'a, R>(
		&'a self,
		f: impl FnMut(Option<&RCSlot<T, Interface>>) -> R + 'a,
	) -> impl Iterator<Item = R> + ExactSizeIterator + 'a {
		let reaper_include = |_, slot: &Slot<T>| {
			// Safety: Reaper state ensures free_timestamp has been written
			let free_timestamp = unsafe { slot.free_timestamp.with(|t| *t) };
			free_timestamp.compare_wrapping(&self.lock_timestamp).unwrap().is_ge()
		};
		// Safety: Only reapered slots that are protected by this lock are accessible
		unsafe { self.slots.iter_with(reaper_include, f) }
	}

	// FIXME: unsound: slots may be accessed that are freed concurrently
	// /// Same functionality as iter_last
	// pub fn iter_latest<'a, R>(
	// 	this: &'a Arc<Self>,
	// 	_lock: &AtomicRCSlotsLock<T>,
	// 	mut f: impl FnMut(Option<&RCSlot<T>>) -> R + 'a,
	// ) {
	// 	self.iter_with()
	// }
}

impl<T, Interface: RCSlotsInterface<T>> Drop for Lock<T, Interface> {
	fn drop(&mut self) {
		self.slots.unlock(self.lock_timestamp);
	}
}

pub struct RCSlots<T, Interface: RCSlotsInterface<T> = DefaultRCSlotInterface> {
	interface: Interface,
	array: Box<[Slot<T>]>,
	next_free: CachePadded<AtomicUsize>,
	/// queue of slots that can be dropped and reclaimed, as soon as all locks that may be using them have finished
	reaper_queue: SegQueue<SlotIndex>,
	/// queue of slots that are dead and may be reused
	dead_queue: SegQueue<SlotIndex>,
	/// Every created lock gets a new timestamp from this counter. Current active locks is `next_lock_counter.wrapping_sub(finished_lock_counter)`. May wrap around.
	lock_timestamp_curr: CachePadded<AtomicU32>,
	/// Counter to track the amount of locks that have been unlocked. Current active locks is `next_lock_counter.wrapping_sub(finished_lock_counter)`. May wrap around.
	unlock_timestamp_curr: CachePadded<AtomicU32>,
	/// A RangeSet containing all locks that have been unlocked, that are *greater than* `unlock_timestamp_curr` and
	/// thus must wait for a previous lock to unlock before being able to clean up their resources.
	///
	/// Note: A RangeMap does not support wrapping numbers, but should be fine for now. Will fix when it causes issues.
	unlock_future_timestamp: CachePadded<Mutex<RangeSet<Timestamp>>>,
	/// Contains `UNLOCK_CONTROL_*` bitflags to sync unlock logic between threads without causing stalls.
	unlock_control: CachePadded<AtomicU32>,
}

const UNLOCK_CONTROL_LOCKED: u32 = 0x1;
const UNLOCK_CONTROL_MORE: u32 = 0x01;

impl<T> RCSlots<T, DefaultRCSlotInterface> {
	pub fn new(len: usize) -> Arc<Self> {
		Self::new_with_interface(len, DefaultRCSlotInterface {})
	}
}

impl<T, Interface: RCSlotsInterface<T>> RCSlots<T, Interface> {
	pub fn new_with_interface(len: usize, interface: Interface) -> Arc<Self> {
		Arc::new(Self {
			interface,
			array: (0..len).map(|_| Slot::default()).collect::<Vec<_>>().into_boxed_slice(),
			next_free: CachePadded::new(AtomicUsize::new(0)),
			reaper_queue: SegQueue::new(),
			dead_queue: SegQueue::new(),
			lock_timestamp_curr: CachePadded::new(AtomicU32::new(0)),
			unlock_timestamp_curr: CachePadded::new(AtomicU32::new(0)),
			unlock_future_timestamp: CachePadded::new(Mutex::new(RangeSet::new())),
			unlock_control: CachePadded::new(AtomicU32::new(0)),
		})
	}

	pub fn allocate(self: &Arc<Self>, t: T) -> RCSlot<T, Interface> {
		let index = self
			.dead_queue
			.pop()
			.unwrap_or_else(|| SlotIndex(self.next_free.fetch_add(1, Relaxed)));

		let slot = &self.array[index];
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

		// Safety: transfer ownership of the ref increment done above
		unsafe { RCSlot::new(Arc::into_raw(self.clone()), index) }
	}

	/// # Safety
	/// must only be called when the last RC was dropped, so we can acquire exclusive ownership
	#[cold]
	#[inline(never)]
	unsafe fn slot_starts_dying(&self, index: SlotIndex) {
		let slot = &self.array[index];
		let curr = Timestamp::new(self.lock_timestamp_curr.load(Relaxed));

		// FIXME separate these two timestamp queries:
		// 	* First, the curr queries the current timestamp, at which this slot can be freed.
		//  * **Then** check if it can be freed immediately, which is completely separate from above!
		let finished = Timestamp::new(self.unlock_timestamp_curr.load(Acquire));
		// free_now means there are no locks present
		let free_now = curr.compare_wrapping(&finished).unwrap().is_le();

		if free_now {
			// FIXME freeing may race against CleanupLock, fix this!!!!
			unsafe {
				self.free_slot(index, Alive);
			}
			// put onto dead queue
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.dead_queue.push(index);
		} else {
			// SAFETY: while the slot is (still) alive, no-one is allowed to hold a reference against free_timestamp, thus grabbing a mutable ref is safe.
			// Also, the method contract ensures only one thread may enter this method for each alive slot.
			unsafe {
				slot.free_timestamp.with_mut(|free_timestamp| *free_timestamp = curr);
			}

			// free_timestamp was set and may now be read by others, like lock iteration
			slot.version_swap(Alive, Reaper, Release);

			// put onto reaper queue
			// the order they are put into the queue may differ from their timestamps, so a few entries may get stuck
			// afterward, we NO LONGER have exclusive ownership, so no with_mut() allowed!
			self.reaper_queue.push(index);
		}
	}

	/// Frees the alive slot and turns it dead. Panics if the slot was not alive.
	///
	/// # Safety
	/// Must have exclusive access to `slot.t`
	#[inline]
	unsafe fn free_slot(&self, index: SlotIndex, from: VersionState) {
		let slot = &self.array[index];
		slot.version_swap(from, Dead, Relaxed);
		// Safety: nobody should be accessing t, so ordering of version_swap does not matter
		unsafe {
			slot.t.with_mut(|t| {
				// drops t and makes it uninit
				self.interface.drop_slot(index, t.assume_init_read())
			});
		}
	}

	pub fn lock(self: &Arc<Self>) -> Lock<T, Interface> {
		let lock_id = Timestamp(Wrapping(self.lock_timestamp_curr.fetch_add(1, Relaxed)) + Wrapping(1));
		Lock {
			slots: self.clone(),
			lock_timestamp: lock_id,
		}
	}

	fn unlock(self: &Arc<Self>, lock_timestamp: Timestamp) {
		{
			let mut guard = self.unlock_future_timestamp.lock();
			guard.insert(lock_timestamp..Timestamp(lock_timestamp.0 + Wrapping(1)));
		}

		let result = self
			.unlock_control
			.fetch_or(UNLOCK_CONTROL_LOCKED | UNLOCK_CONTROL_MORE, Relaxed);
		let acq_lock = result & UNLOCK_CONTROL_LOCKED == 0;
		if !acq_lock {
			// whoever has the lock will do the cleanup
			return;
		}

		self.cleanup_unlock();
	}

	fn cleanup_unlock(&self) {
		let mut unlock_timestamp = Timestamp::new(self.unlock_timestamp_curr.load(Relaxed));
		loop {
			{
				// only lock while figuring out timestamps, not while dropping slots
				let mut guard = self.unlock_future_timestamp.lock();
				// clear UNLOCK_CONTROL_MORE flag
				self.unlock_control.fetch_and(UNLOCK_CONTROL_LOCKED, Relaxed);

				while let Some(range) = guard.get(&Timestamp(unlock_timestamp.0 + Wrapping(1))) {
					let range = range.clone();
					unlock_timestamp = Timestamp(range.end.0 - Wrapping(1));
					guard.remove(range);
				}
			}
			self.unlock_timestamp_curr.store(unlock_timestamp.0 .0, Relaxed);

			while let Some(index) = self.reaper_queue.pop() {
				let slot = &self.array[index];
				// Safety: slots in the reaper queue must be in Reaper state, and thus have the timestamp initialized
				// and readable by shared ref
				let free_timestamp = unsafe { slot.free_timestamp.with(|t| *t) };
				// TODO move this to docs
				// It is required to be less_than and not just equal, as some entries may get stuck due to entries being
				// added out of order compared to their timestamp. Thus, we also have to free previously unlocked
				// entries, which have gone stuck. But there is a risk: Constant locking without any new entries to
				// flush the queue can cause timestamps to wrap around, and then we don't know if it was before or after
				// us!
				if free_timestamp
					.compare_wrapping(&unlock_timestamp)
					.expect("Reaper queue stood still for too long, timestamps have wrapped around!")
					.is_le()
				{
					// Safety: we have exclusive access to this slot, and just verified that the last lock access to
					// this slot has dropped, so we may drop the slot
					unsafe { self.free_slot(index, Reaper) };
					self.dead_queue.push(index);
				} else {
					break;
				}
			}

			// unlock, or retry if MORE flag was set
			if self
				.unlock_control
				.compare_exchange(UNLOCK_CONTROL_LOCKED, 0, Relaxed, Relaxed)
				.is_ok()
			{
				break;
			}
		}
	}

	/// The amount of slots that have been allocated until now. Should immediately be considered
	/// outdated, but is guaranteed to only ever monotonically increase.
	#[inline]
	pub fn slots_allocated(&self) -> usize {
		self.next_free.load(Relaxed)
	}

	/// Iterates through all slots
	///
	/// # Safety
	/// Caller must ensure that slots for which reaper_include() returns true are not dropped while iterating
	unsafe fn iter_with<'a, R>(
		self: &'a Arc<Self>,
		reaper_include: impl Fn(SlotIndex, &Slot<T>) -> bool + 'a,
		mut f: impl FnMut(Option<&RCSlot<T, Interface>>) -> R + 'a,
	) -> impl Iterator<Item = R> + ExactSizeIterator + 'a {
		let max = self.next_free.load(Relaxed);

		(0..max).map(move |index| {
			let index = SlotIndex(index);
			let slot = &self.array[index];
			let present = match VersionState::from(slot.version.load(Relaxed)).0 {
				Dead => false,
				Alive => true,
				Reaper => reaper_include(index, slot),
			};

			if present {
				// Safety: we actually do NOT transfer ownership of a ref_count here, instead we never drop the RCSlot
				let rc_slot = unsafe { ManuallyDrop::new(RCSlot::new(Arc::as_ptr(self), index)) };
				f(Some(&rc_slot))
			} else {
				f(None)
			}
		})
	}
}

#[cfg(test)]
mod test_utils {
	use std::mem::replace;

	use super::*;

	pub struct LockUnlock<T, Interface: RCSlotsInterface<T>> {
		slots: Arc<RCSlots<T, Interface>>,
		lock: Lock<T, Interface>,
	}

	impl<T, Interface: RCSlotsInterface<T>> LockUnlock<T, Interface> {
		pub fn new(slots: &Arc<RCSlots<T, Interface>>) -> Self {
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
	use crate::rc_slots::rc_slots::test_utils::LockUnlock;

	use super::*;

	#[test]
	fn test_ref_counting() {
		let slots = RCSlots::new(32);
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
		let slots = RCSlots::new(32);
		let slot = slots.allocate(42);
		let slot2 = slot.clone();

		// Safety: this is not safe, and should cause a panic later
		unsafe { slot.ref_dec() };
		// need 2 slots otherwise we leak memory
		drop(slot2);
		drop(slot);
	}

	#[test]
	fn test_alloc_unique() {
		let slots = RCSlots::new(32);

		let count: u32 = 5;
		let vec = (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();
		for (i, slot) in vec.iter().enumerate() {
			assert_eq!(slot.deref_copy(), i as u32);
			assert_eq!(slots.array.key_to_raw_index(slot.index), i as u32);

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
			let slots = RCSlots::new(32);
			let mut lock_unlock = LockUnlock::new(&slots);

			let arc1 = Arc::new(42);
			let slot1 = slots.allocate(arc1.clone());
			assert_eq!(slots.array.key_to_raw_index(slot1.index), 0);
			let arc2 = Arc::new(69);
			let slot2 = slots.allocate(arc2.clone());
			assert_eq!(slots.array.key_to_raw_index(slot2.index), 1);

			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // alive
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			drop(slot1);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // alive
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // reaper
			assert_eq!(Arc::strong_count(&arc2), 2); // alive

			drop(slot2);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 2);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);
			assert_eq!(Arc::strong_count(&arc1), 2); // reaper
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			assert_eq!(Arc::strong_count(&arc1), 1); // dead
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper

			// we asserted that it was freed, useless now
			drop(arc1);

			// new slot will allocate a new slot, not reuse, as dead_queue retains 1 slot
			let arc3 = Arc::new(3);
			let slot3 = slots.allocate(arc3.clone());
			assert_eq!(slots.array.key_to_raw_index(slot3.index), 2);

			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper
			assert_eq!(Arc::strong_count(&arc3), 2); // alive

			drop(slot3);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 2);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			assert_eq!(Arc::strong_count(&arc2), 2); // reaper
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper

			// reaper_queue retains 1 slot
			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 2);
			assert_eq!(Arc::strong_count(&arc2), 1); // dead
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper

			// we asserted that it was freed, useless now
			drop(arc2);

			// new slot will reuse slot 1
			let arc4 = Arc::new(4);
			let slot4 = slots.allocate(arc4.clone());
			assert_eq!(slots.array.key_to_raw_index(slot4.index), 0);

			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper
			assert_eq!(Arc::strong_count(&arc4), 2); // alive

			// new slot will allocate a new slot, not reuse, as dead_queue retains 1 slot
			let arc5 = Arc::new(5);
			let slot5 = slots.allocate(arc5.clone());
			assert_eq!(slots.array.key_to_raw_index(slot5.index), 3);

			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			assert_eq!(Arc::strong_count(&arc3), 2); // reaper
			assert_eq!(Arc::strong_count(&arc4), 2); // alive
			assert_eq!(Arc::strong_count(&arc5), 2); // alive
		}
	}

	// TODO test slot version!
	#[test]
	fn test_queues_many_entries() {
		unsafe {
			let slots = RCSlots::new(32);
			let mut lock_unlock = LockUnlock::new(&slots);

			for i in 0..5 {
				let slot = slots.allocate(());
				assert_eq!(slots.array.key_to_raw_index(slot.index), i);
			}
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 5);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);

			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 4);

			// 3 reused
			for i in 0..3 {
				let slot = slots.allocate(());
				assert_eq!(slots.array.key_to_raw_index(slot.index), i);
				assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1 + i);
				assert_eq!(slots.dead_queue.debug_count(&slots.array), 3 - i);
			}

			// 2 newly allocated
			for i in 0..2 {
				let slot = slots.allocate(());
				assert_eq!(slots.array.key_to_raw_index(slot.index), i + 5);
				assert_eq!(slots.reaper_queue.debug_count(&slots.array), 4 + i);
				assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);
			}

			lock_unlock.advance();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 6);
		}
	}

	#[test]
	fn test_queues_while_unlocked() {
		unsafe {
			let slots = RCSlots::new(32);

			drop(slots.allocate(0));
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			let slot = slots.allocate(1);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			drop(slot);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 2);

			let slot = slots.allocate(2);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			drop(slot);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 2);
		}
	}

	#[test]
	fn test_queues_mix_locked_and_unlocked() {
		unsafe {
			let slots = RCSlots::new(32);
			let alloc = |count: u32| (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();

			// unlocked behaviour
			// 5 new
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 5);

			// locked behaviour
			let lock = slots.lock();
			// 1 new, 4 reused
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 5);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			lock.unlock();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 5);

			// unlocked behaviour
			// 1 new, 4 reused
			let vec = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 1);

			drop(vec);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 6);
		}
	}

	#[test]
	fn test_queues_drop_before_and_after_lock() {
		unsafe {
			let slots = RCSlots::new(32);
			let alloc = |count: u32| (0..count).map(|i| slots.allocate(i)).collect::<Vec<_>>();

			let before_lock_a = alloc(2);
			let before_lock_b = alloc(3);
			let lock = slots.lock();
			let after_lock_a = alloc(4);
			let after_lock_b = alloc(5);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 0);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);

			drop(before_lock_a);
			drop(after_lock_a);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 6);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 0);

			lock.unlock();
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 5);

			drop(before_lock_b);
			drop(after_lock_b);
			assert_eq!(slots.reaper_queue.debug_count(&slots.array), 1);
			assert_eq!(slots.dead_queue.debug_count(&slots.array), 13);
		}
	}

	#[test]
	fn test_reaper_queue_leak() {
		let slots = RCSlots::new(32);

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
		let slots = RCSlots::new(32);

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

	fn iter_collect<T: Clone, Interface: RCSlotsInterface<T>>(lock: &Lock<T, Interface>) -> Vec<Option<T>> {
		lock.iter_with(|t| t.map(|slot| (**slot).clone())).collect::<Vec<_>>()
	}

	#[test]
	fn test_iter_smoke() {
		let slots = RCSlots::new(32);
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
		let slots = RCSlots::new(32);
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

	#[test]
	#[should_panic(expected = "(state: Reaper) differed from expected state Alive!")]
	fn test_iter_clone() {
		let slots = RCSlots::new(32);
		let slot1 = slots.allocate(42);

		let lock1 = slots.lock();
		assert_eq!(iter_collect(&lock1), [Some(42)]);

		drop(slot1);
		assert_eq!(iter_collect(&lock1), [Some(42)]);

		// necromancy is not allowed!
		lock1.iter_with(|o| o.map(|s| s.clone())).next();
	}
}
