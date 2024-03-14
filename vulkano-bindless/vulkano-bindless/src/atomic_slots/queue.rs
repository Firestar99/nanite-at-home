use std::marker::PhantomData;

use crate::atomic_slots::{AtomicSlots, SlotKey};
use crate::sync::atomic::AtomicU32;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::Mutex;
use crate::sync::MutexGuard;
use crate::sync::SpinWait;

pub struct SlotChain {
	head: SlotKey,
	tail: SlotKey,
}

impl SlotChain {
	#[inline]
	fn new(head: SlotKey, tail: SlotKey) -> Self {
		Self { head, tail }
	}

	pub fn single_slot_key(&self) -> Option<SlotKey> {
		if self.head == self.tail {
			Some(self.head)
		} else {
			None
		}
	}
}

impl From<SlotKey> for SlotChain {
	#[inline]
	fn from(value: SlotKey) -> Self {
		SlotChain::new(value, value)
	}
}

pub trait QueueSlot: Default {
	fn atomic(&self) -> &AtomicU32;
}

pub struct BaseQueue<S: QueueSlot> {
	tail: AtomicU32,
	_phantom: PhantomData<S>,
}

impl<S: QueueSlot> BaseQueue<S> {
	pub fn new() -> Self {
		Self {
			tail: AtomicU32::new(!0),
			_phantom: PhantomData {},
		}
	}

	/// Push a chain of [`SlotKey`]s onto the queue.
	/// Returns Some() if the queue has dried up and the head needs to be reconnected, with the SlotKey and associated index of the slot it should connect to.
	#[inline]
	pub fn push_chain(&self, atomic_slots: &AtomicSlots<S>, chain: SlotChain) -> Option<(SlotKey, u32)> {
		let head_index = atomic_slots.key_to_raw_index(chain.head);
		let tail_index = atomic_slots.key_to_raw_index(chain.tail);
		// set `next` of this slot to !0 aka no next key
		atomic_slots.with(chain.tail, |slot| slot.atomic().store(!0, Relaxed));
		// Release is enough as we won't load anything, just store the `next` index atomically
		let prev_index = self.tail.swap(tail_index, Release);

		if prev_index != !0 {
			// Safety: we inserted it, it must be valid
			let prev_key = unsafe { atomic_slots.key_from_raw_index(prev_index) };
			atomic_slots.with(prev_key, |prev_slot| prev_slot.atomic().store(head_index, Relaxed));
			None
		} else {
			Some((chain.head, head_index))
		}
	}
}

pub trait Queue<S: QueueSlot> {
	fn push(&self, atomic_slots: &AtomicSlots<S>, slot_key: SlotKey);
	fn push_chain(&self, atomic_slots: &AtomicSlots<S>, chain: SlotChain);
	fn pop(&self, atomic_slots: &AtomicSlots<S>) -> Option<SlotKey>;
}

pub struct PopQueue<S: QueueSlot> {
	base: BaseQueue<S>,
	head: AtomicU32,
}

impl<S: QueueSlot> PopQueue<S> {
	pub fn new() -> Self {
		Self {
			base: BaseQueue::new(),
			head: AtomicU32::new(!0),
		}
	}
}

impl<S: QueueSlot> Queue<S> for PopQueue<S> {
	fn push(&self, atomic_slots: &AtomicSlots<S>, slot_key: SlotKey) {
		self.push_chain(atomic_slots, slot_key.into())
	}

	fn push_chain(&self, atomic_slots: &AtomicSlots<S>, chain: SlotChain) {
		if let Some((_, head)) = self.base.push_chain(atomic_slots, chain) {
			self.head.store(head, Relaxed);
		}
	}

	fn pop(&self, atomic_slots: &AtomicSlots<S>) -> Option<SlotKey> {
		let mut spin_wait = SpinWait::new();

		loop {
			let slot_index = self.head.load(Acquire);
			if slot_index == !0 {
				return None;
			}

			// Safety: we inserted it, it must be valid
			let slot_key = unsafe { atomic_slots.key_from_raw_index(slot_index) };
			let next_index = atomic_slots.with(slot_key, |slot| slot.atomic().load(Relaxed));
			if next_index == !0 {
				return None;
			}

			if let Ok(_) = self
				.head
				.compare_exchange_weak(slot_index, next_index, Relaxed, Relaxed)
			{
				return Some(slot_key);
			}

			spin_wait.spin();
		}
	}
}

pub struct ChainQueue<S: QueueSlot> {
	base: BaseQueue<S>,
	head: Mutex<Option<SlotKey>>,
}

impl<S: QueueSlot> ChainQueue<S> {
	pub fn new() -> Self {
		Self {
			base: BaseQueue::new(),
			head: Mutex::new(None),
		}
	}

	pub fn pop_chain_inner(
		&self,
		atomic_slots: &AtomicSlots<S>,
		mut process_slot: impl FnMut(SlotKey) -> bool,
		lock: &mut MutexGuard<Option<SlotKey>>,
	) -> Option<SlotChain> {
		if let Some(head) = **lock {
			let mut prev = head;
			let mut curr = head;
			loop {
				let next_index = atomic_slots.with(curr, |slot| slot.atomic().load(Acquire));
				if next_index == !0 {
					break;
				}

				prev = curr;
				// Safety: we inserted it, it must be valid
				curr = unsafe { atomic_slots.key_from_raw_index(next_index) };
				if !process_slot(prev) {
					break;
				}
			}

			if curr != head {
				**lock = Some(curr);
				return Some(SlotChain::new(head, prev));
			} else {
				None
			}
		} else {
			None
		}
	}

	pub fn try_pop_chain(
		&self,
		atomic_slots: &AtomicSlots<S>,
		process_slot: impl FnMut(SlotKey) -> bool,
	) -> Option<SlotChain> {
		if let Some(mut lock) = self.head.try_lock() {
			self.pop_chain_inner(atomic_slots, process_slot, &mut lock)
		} else {
			None
		}
	}

	pub fn pop_chain(
		&self,
		atomic_slots: &AtomicSlots<S>,
		process_slot: impl FnMut(SlotKey) -> bool,
	) -> Option<SlotChain> {
		let mut lock = self.head.lock();
		self.pop_chain_inner(atomic_slots, process_slot, &mut lock)
	}
}

impl<S: QueueSlot> Queue<S> for ChainQueue<S> {
	fn push(&self, atomic_slots: &AtomicSlots<S>, slot_key: SlotKey) {
		self.push_chain(atomic_slots, slot_key.into())
	}

	fn push_chain(&self, atomic_slots: &AtomicSlots<S>, chain: SlotChain) {
		if let Some((head, _)) = self.base.push_chain(atomic_slots, chain) {
			*self.head.lock() = Some(head);
		}
	}

	fn pop(&self, atomic_slots: &AtomicSlots<S>) -> Option<SlotKey> {
		self.pop_chain(atomic_slots, |_| false)
			.map(|chain| chain.single_slot_key().unwrap())
	}
}

#[cfg(test)]
mod test_helper {
	use super::*;

	#[derive(Debug, Default)]
	pub struct TestSlot {
		atomic: AtomicU32,
	}

	impl QueueSlot for TestSlot {
		fn atomic(&self) -> &AtomicU32 {
			&self.atomic
		}
	}
}

#[cfg(all(test, not(feature = "loom_tests")))]
mod tests {
	use rand::prelude::SliceRandom;
	use rand::rngs::mock::StepRng;

	use crate::atomic_slots::queue::test_helper::TestSlot;

	use super::*;

	#[test]
	fn test_pop_empty() {
		test_empty(PopQueue::new());
	}

	#[test]
	fn test_chain_empty() {
		test_empty(ChainQueue::new());
	}

	fn test_empty(queue: impl Queue<TestSlot>) {
		let slots = AtomicSlots::new(32);
		assert_eq!(queue.pop(&slots), None);
	}

	#[test]
	fn test_pop_single() {
		test_single(PopQueue::new());
	}

	#[test]
	fn test_chain_single() {
		test_single(ChainQueue::new());
	}

	fn test_single(queue: impl Queue<TestSlot>) {
		let slots = AtomicSlots::new(32);
		let key0 = slots.allocate();
		let key1 = slots.allocate();

		queue.push(&slots, key0);
		queue.push(&slots, key1);
		assert_eq!(queue.pop(&slots), Some(key0));
		assert_eq!(queue.pop(&slots), None);
	}

	#[test]
	fn test_pop_many() {
		test_many(PopQueue::new());
	}

	#[test]
	fn test_chain_many() {
		test_many(ChainQueue::new());
	}

	fn test_many(queue: impl Queue<TestSlot>) {
		let slots = AtomicSlots::new(32);

		const KEY_COUNT: usize = 5;
		let mut keys = [(); KEY_COUNT].map(|_| slots.allocate());
		let mut rng = StepRng::new(!1, 0xDEADBEEF);
		keys.shuffle(&mut rng);
		println!("{:?}", keys);

		for key in keys {
			queue.push(&slots, key);
		}

		// last entry cannot be poped
		for key in &keys[0..KEY_COUNT - 1] {
			assert_eq!(queue.pop(&slots), Some(*key));
		}
		assert_eq!(queue.pop(&slots), None);
	}

	#[test]
	fn test_pop_dry_and_reconnect() {
		test_dry_and_reconnect(PopQueue::new());
	}

	#[test]
	fn test_chain_dry_and_reconnect() {
		test_dry_and_reconnect(ChainQueue::new());
	}

	fn test_dry_and_reconnect(queue: impl Queue<TestSlot>) {
		let slots = AtomicSlots::new(32);

		let key0 = slots.allocate();
		let key1 = slots.allocate();
		let key2 = slots.allocate();

		assert_eq!(queue.pop(&slots), None);

		queue.push(&slots, key0);
		assert_eq!(queue.pop(&slots), None);

		queue.push(&slots, key1);
		assert_eq!(queue.pop(&slots), Some(key0));
		assert_eq!(queue.pop(&slots), None);

		queue.push(&slots, key2);
		assert_eq!(queue.pop(&slots), Some(key1));
		assert_eq!(queue.pop(&slots), None);
	}

	#[test]
	fn test_make_chain() {
		let slots = AtomicSlots::<TestSlot>::new(32);
		make_chain(&slots, 5);
	}

	fn make_chain(slots: &AtomicSlots<TestSlot>, key_count: usize) -> (SlotChain, Vec<SlotKey>) {
		let first = ChainQueue::new();

		let mut keys: Vec<_> = (0..key_count).map(|_| slots.allocate()).collect();
		let mut rng = StepRng::new(!1, 0xDEADBEEF);
		keys.shuffle(&mut rng);
		println!("{:?}", keys);

		for key in &keys {
			first.push(&slots, *key);
		}
		first.push(&slots, slots.allocate());

		let chain = first.pop_chain(&slots, |_| true).unwrap();
		assert_eq!(chain.head, keys[0]);
		assert_eq!(chain.tail, keys[key_count - 1]);
		(chain, keys)
	}

	#[test]
	fn test_pop_push_chain() {
		test_push_chain(PopQueue::new());
	}

	#[test]
	fn test_chain_push_chain() {
		test_push_chain(ChainQueue::new());
	}

	fn test_push_chain(queue: impl Queue<TestSlot>) {
		let slots = AtomicSlots::new(32);
		let (chain, keys) = make_chain(&slots, 8);

		// actual test
		queue.push_chain(&slots, chain);
		queue.push(&slots, slots.allocate());
		for key in &keys {
			assert_eq!(queue.pop(&slots), Some(*key));
		}
		assert_eq!(queue.pop(&slots), None);
	}
}

#[cfg(test)]
mod loom_tests {
	use crate::atomic_slots::queue::test_helper::TestSlot;
	use crate::sync::loom::*;
	use crate::sync::thread::spawn;
	use crate::sync::Arc;

	use super::*;

	#[test]
	fn test_pop_push() {
		test_push(|| PopQueue::new())
	}

	#[test]
	fn test_chain_push() {
		test_push(|| ChainQueue::new())
	}

	fn test_push<Q>(queue: impl Fn() -> Q + Send + Sync + 'static)
	where
		Q: Queue<TestSlot> + Send + Sync + 'static,
	{
		const PUSH_THREADS: usize = 3;
		model_builder(
			|b| b.preemption_bound = Some(4),
			move || {
				let queue = Arc::new(queue());
				let slots = Arc::new(AtomicSlots::new(32));
				launch_loom_threads((0..PUSH_THREADS).map(|_| {
					let slots = slots.clone();
					let queue = queue.clone();
					let key = slots.allocate();
					move || {
						queue.push(&slots, key);
					}
				}));
			},
		);
	}

	#[test]
	fn test_pop_pop() {
		test_pop(|| PopQueue::new())
	}

	#[test]
	fn test_chain_pop() {
		test_pop(|| ChainQueue::new())
	}

	fn test_pop<Q>(queue: impl Fn() -> Q + Send + Sync + 'static)
	where
		Q: Queue<TestSlot> + Send + Sync + 'static,
	{
		const POP_THREADS: usize = 3;
		model_builder(
			|b| b.preemption_bound = Some(4),
			move || {
				let queue = Arc::new(queue());
				let slots = Arc::new(AtomicSlots::new(32));
				let keys = [(); POP_THREADS].map(|_| slots.allocate());
				for key in keys {
					queue.push(&slots, key);
				}
				queue.push(&slots, slots.allocate());

				let iter = (0..POP_THREADS).map(|_| {
					let slots = slots.clone();
					let queue = queue.clone();
					move || queue.pop(&slots).unwrap()
				});
				let mut result = launch_loom_threads_and_wait(iter);

				result.sort_by_key(|key| slots.key_to_raw_index(*key));
				assert_eq!(result, keys);
			},
		);
	}

	// loom does not like push_pop at all, generates way too many branches for some reason, which cause int overflows
	#[test]
	#[cfg_attr(feature = "loom_tests", ignore)]
	fn test_pop_push_pop() {
		test_push_pop(|| PopQueue::new())
	}

	#[test]
	#[cfg_attr(feature = "loom_tests", ignore)]
	fn test_chain_push_pop() {
		test_push_pop(|| ChainQueue::new())
	}

	fn test_push_pop<Q>(queue: impl Fn() -> Q + Send + Sync + 'static)
	where
		Q: Queue<TestSlot> + Send + Sync + 'static,
	{
		const THREADS: usize = 1;
		model_builder(
			|b| {
				b.expect_explicit_explore = true;
				b.max_branches = 100000;
				b.preemption_bound = Some(1);
			},
			move || {
				let queue = Arc::new(queue());
				let slots = Arc::new(AtomicSlots::new(32));
				let keys = [(); THREADS + 1].map(|_| slots.allocate());
				queue.push(&slots, keys[THREADS]);

				let push = (0..THREADS)
					.map(|i| {
						let slots = slots.clone();
						let queue = queue.clone();
						move || {
							queue.push(&slots, keys[i]);
						}
					})
					.collect::<Vec<_>>();

				let pop = (0..THREADS)
					.map(|_| {
						let slots = slots.clone();
						let queue = queue.clone();
						move || {
							let mut spin_wait = SpinWait::new();
							loop {
								if let Some(slot) = queue.pop(&slots) {
									return slot;
								}
								spin_wait.spin();
							}
						}
					})
					.collect::<Vec<_>>();

				explore();
				for x in push {
					spawn(x);
				}
				let mut result = launch_loom_threads_and_wait(pop.into_iter());
				stop_exploring();

				// empty out queue
				result.push({
					queue.push(&slots, slots.allocate());
					// Normally pop should be unable to return None, as the push above by the same thread ensures it can be poped. But loom says otherwise.
					let mut spin_wait = SpinWait::new();
					loop {
						if let Some(key) = queue.pop(&slots) {
							break key;
						} else {
							spin_wait.spin();
						}
					}
				});

				result.sort_by_key(|key| slots.key_to_raw_index(*key));
				assert_eq!(result, keys);
			},
		);
	}
}
