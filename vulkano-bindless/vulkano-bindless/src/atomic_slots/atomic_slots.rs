use std::mem::MaybeUninit;
use std::sync::atomic::AtomicU32;

use static_assertions::{assert_eq_size, const_assert_eq};

use crate::sync::atomic::AtomicU16;
use crate::sync::atomic::Ordering::*;
use crate::sync::cell::UnsafeCell;
use crate::sync::SpinWait;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SlotKey {
	instance_id: u16,
	block: u16,
	slot: u32,
}

assert_eq_size!(SlotKey, u64);

const BLOCK_COUNT: usize = 32;

/// A concurrent SlotMap that does not reallocate its slots.
pub struct AtomicSlots<V: Default> {
	blocks: [UnsafeCell<MaybeUninit<Box<[V]>>>; BLOCK_COUNT],
	next_free_slot: AtomicU32,
	block_size_log: u32,
	blocks_allocated: AtomicU16,
	instance_id: u16,
}

unsafe impl<V: Default> Send for AtomicSlots<V> {}
unsafe impl<V: Default> Sync for AtomicSlots<V> {}

const BLOCKS_ALLOCATED_COUNT_MASK: u16 = 0x7FFF;
const BLOCKS_ALLOCATED_ALLOCATING_FLAG: u16 = 0x8000;
const_assert_eq!(BLOCKS_ALLOCATED_COUNT_MASK & BLOCKS_ALLOCATED_ALLOCATING_FLAG, 0);

// FIXME: verify all orderings, add concurrency tests, add free method
impl<V: Default> AtomicSlots<V> {
	/// Creates a new [`AtomicSlots`] instance.
	/// `first_block_size` must be larger than 0 and will be rounded down to the next power of two. It indicates the size of the first block,
	/// whereas the next block will always be double the size of the previous one.
	pub fn new(first_block_size: u32) -> Self {
		let block_size_log = first_block_size.checked_ilog2().expect("Block size may not be zero");

		#[cfg(not(feature = "loom"))]
		let instance_id = {
			static INSTANCE_COUNTER: AtomicU16 = AtomicU16::new(0);
			INSTANCE_COUNTER.fetch_add(1, Relaxed)
		};
		#[cfg(feature = "loom")]
		let instance_id = 0;

		Self {
			blocks: [0; BLOCK_COUNT].map(|_| UnsafeCell::new(MaybeUninit::uninit())),
			next_free_slot: AtomicU32::new(0),
			block_size_log,
			blocks_allocated: AtomicU16::new(0),
			instance_id,
		}
	}

	pub fn allocate(&self) -> SlotKey {
		// allocate a new slot
		// Safety: we just allocated it from self
		let slot = unsafe { self.key_from_raw_index(self.next_free_slot.fetch_add(1, Relaxed)) };

		let mut spin_wait = SpinWait::new();
		loop {
			// block already allocated, may need to be acquired
			let blocks_allocated = self.blocks_allocated.load(Acquire) & BLOCKS_ALLOCATED_COUNT_MASK;
			if slot.block < blocks_allocated {
				return slot;
			}

			// try to allocate a new memory block
			let blocks_allocated_new = blocks_allocated | BLOCKS_ALLOCATED_ALLOCATING_FLAG;
			match self
				.blocks_allocated
				.compare_exchange_weak(blocks_allocated, blocks_allocated_new, Relaxed, Relaxed)
			{
				// this thread is the one to allocate a new memory block
				Ok(_) => {
					for i in blocks_allocated..=slot.block {
						// SAFETY: the struct is locked to ensure we are the only ones allocating, and indices larger than blocks_allocated should never be
						// accessed anywhere before this has passed
						unsafe {
							// TODO allocation panic may cause a deadlock
							let block = (0..(1 << (i as u32 + self.block_size_log)))
								.map(|_| V::default())
								.collect::<Vec<_>>()
								.into_boxed_slice();
							self.blocks.get(i as usize).unwrap().with_mut(|block_ref| {
								block_ref.write(block);
							});
						}
					}
					// also unlocks
					self.blocks_allocated.store(slot.block + 1, Release);
					return slot;
				}
				// spin and retry
				Err(_) => {}
			}

			spin_wait.spin();
		}
	}

	/// Create a SlotKey from a raw index.
	///
	/// # Safety
	/// The index must have been allocated using [`Self::allocate`] on this instance.
	pub unsafe fn key_from_raw_index(&self, index: u32) -> SlotKey {
		assert!(index < (u32::MAX - 100), "Out of slots!");

		let base_offset = 1u64 << self.block_size_log;
		let shift = (index as u64 + base_offset).ilog2();
		let block = shift - self.block_size_log;

		let slot_offset = (1u64 << shift) - base_offset;
		let slot = index - slot_offset as u32;

		SlotKey {
			instance_id: self.instance_id,
			block: block.try_into().unwrap(),
			slot: slot.try_into().unwrap(),
		}
	}

	pub fn key_to_raw_index(&self, key: SlotKey) -> u32 {
		self.check(key);
		let base_offset = 1u64 << self.block_size_log;
		let shift = key.block as u32 + self.block_size_log;
		let slot_offset = (1u64 << shift) - base_offset;
		key.slot + slot_offset as u32
	}

	pub fn check(&self, slot: SlotKey) {
		assert_eq!(
			slot.instance_id, self.instance_id,
			"SlotIndex used with wrong AtomicSlots instance!"
		);
	}

	pub fn with<R>(&self, slot: SlotKey, f: impl FnOnce(&V) -> R) -> R {
		self.check(slot);
		unsafe { self.blocks[slot.block as usize].with(|block| f(&(&*block).assume_init_ref()[slot.slot as usize])) }
	}
}

/// loom cannot track this correctly
#[cfg(not(feature = "loom"))]
impl<V: Default> core::ops::Index<SlotKey> for AtomicSlots<V> {
	type Output = V;

	fn index(&self, slot: SlotKey) -> &Self::Output {
		self.check(slot);
		unsafe { &(&mut *self.blocks[slot.block as usize].get()).assume_init_ref()[slot.slot as usize] }
	}
}

impl<V: Default> Drop for AtomicSlots<V> {
	fn drop(&mut self) {
		// SAFETY: `self.blocks` has at least as many entries populated as `self.blocks_allocated`
		// (though could be more in case of a panic during allocation)
		unsafe {
			for i in 0..self.blocks_allocated.load(Relaxed) as usize {
				self.blocks[i].with_mut(|block| (&mut *block).assume_init_drop())
			}
		}
	}
}

#[cfg(all(test, not(feature = "loom")))]
mod tests {
	use super::*;

	#[test]
	fn test_make_key() {
		unsafe {
			for (block_size, blocks_max) in [(4, 6), (16, 4), (64, 2)] {
				let slots = AtomicSlots::<u32>::new(block_size);
				let instance_id = slots.instance_id;

				let index_counter = AtomicU32::new(0);
				for block in 0..blocks_max {
					for slot in 0..(block_size << block) {
						let index = index_counter.fetch_add(1, Relaxed);
						let key = slots.key_from_raw_index(index);

						let key_should = SlotKey {
							instance_id,
							block,
							slot,
						};
						assert_eq!(
							key, key_should,
							"key_from_raw_index({}) -> {:?} should be {:?}",
							index, key, key_should
						);

						let index_actual = slots.key_to_raw_index(key);
						assert_eq!(
							index_actual, index,
							"key_to_raw_index({:?}) -> {} should be {}",
							key, index_actual, index
						);
					}
				}
			}
		}
	}

	#[test]
	#[should_panic(expected = "Block size may not be zero")]
	fn test_zero_block_size() {
		AtomicSlots::<u32>::new(0);
	}

	#[test]
	#[should_panic(expected = "SlotIndex used with wrong AtomicSlots instance!")]
	fn test_wrong_atomic_slots() {
		let slots1 = AtomicSlots::<u32>::new(4);
		let slots2 = AtomicSlots::<u32>::new(4);
		let slot = slots1.allocate();
		slots2[slot];
	}

	#[test]
	fn test_alloc_within_block() {
		unsafe {
			let mut slots = AtomicSlots::<u32>::new(16);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
			assert_eq!(slots.allocate(), slots.key_from_raw_index(0));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
			for i in 0..5 {
				assert_eq!(slots.allocate(), slots.key_from_raw_index(i + 1));
			}

			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
			assert_eq!(slots.blocks[0].get_mut().assume_init_ref().len(), 16);
		}
	}

	#[test]
	fn test_alloc_new_block() {
		unsafe {
			let mut slots = AtomicSlots::<u32>::new(4);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
			for i in 0..4 {
				assert_eq!(slots.allocate(), slots.key_from_raw_index(i));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
			}
			assert_eq!(slots.blocks[0].get_mut().assume_init_ref().len(), 4);

			println!("new block!");
			for i in 0..8 {
				assert_eq!(slots.allocate(), slots.key_from_raw_index(i + 4));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 2);
			}
			assert_eq!(slots.blocks[1].get_mut().assume_init_ref().len(), 8);

			println!("new block 2!");
			for i in 0..16 {
				assert_eq!(slots.allocate(), slots.key_from_raw_index(i + 12));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 3);
			}
			assert_eq!(slots.blocks[2].get_mut().assume_init_ref().len(), 16);
		}
	}

	#[test]
	fn test_block_size_not_pow_2() {
		let slots = AtomicSlots::<u32>::new(14);
		assert_eq!(1 << slots.block_size_log, 8);
	}

	#[test]
	fn test_alloc_block_sizes() {
		for (block_size, blocks_max) in [(4, 6), (16, 4), (64, 2)] {
			let mut slots = AtomicSlots::<u32>::new(block_size);

			// fill slots
			{
				let mut curr_block_size = block_size;
				for block in 0..blocks_max {
					assert_eq!(slots.blocks_allocated.load(Relaxed), block);
					for _slot in 0..curr_block_size {
						slots.allocate();
					}
					curr_block_size *= 2;
				}
			}

			unsafe {
				assert_eq!(slots.blocks_allocated.load(Relaxed), blocks_max);

				let mut block_len = block_size as usize;
				for i in 0..blocks_max as usize {
					assert_eq!(
						slots.blocks[i].get_mut().assume_init_ref().len(),
						block_len,
						"block index {} should have the size {} at block_size {}",
						i,
						block_len,
						block_size
					);
					block_len *= 2;
				}
			}
		}
	}
}

#[cfg(test)]
mod loom_tests {
	use crate::sync::loom;
	use crate::sync::Arc;

	use super::*;

	#[test]
	fn loom_alloc_first_block() {
		loom::model(|| {
			let slots = Arc::new(AtomicSlots::<u32>::new(4));
			let slots2 = slots.clone();
			crate::sync::thread::spawn(move || {
				slots2.allocate();
			});
			slots.allocate();
			slots.allocate();
		})
	}

	#[test]
	fn loom_alloc_next_block() {
		loom::model(|| {
			let block_size = 4;
			let slots = Arc::new(AtomicSlots::<u32>::new(block_size));
			for _ in 0..block_size {
				slots.allocate();
			}

			let slots2 = slots.clone();
			crate::sync::thread::spawn(move || {
				slots2.allocate();
			});
			slots.allocate();
		})
	}

	#[test]
	fn loom_access() {
		loom::model(|| {
			let slots = Arc::new(AtomicSlots::<u32>::new(4));
			{
				let slots = slots.clone();
				crate::sync::thread::spawn(move || {
					let key = slots.allocate();
					assert_eq!(slots.with(key, |slot| *slot), 0);
				});
			}
			let key = slots.allocate();
			assert_eq!(slots.with(key, |slot| *slot), 0);
		})
	}
}
