use std::mem::MaybeUninit;

use smallvec::SmallVec;
use static_assertions::{assert_eq_size, const_assert_eq};

use crate::sync::atomic::Ordering::*;
use crate::sync::atomic::{AtomicU16, AtomicU64};
use crate::sync::cell::UnsafeCell;
use crate::sync::SpinWait;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SlotKey {
	instance_id: u16,
	block: u16,
	slot: u32,
}

assert_eq_size!(SlotKey, u64);

const DEFAULT_BLOCK_MAX: usize = 64;

/// A concurrent SlotMap that does not reallocate its slots.
pub struct AtomicSlots<V: Default> {
	blocks: SmallVec<[UnsafeCell<MaybeUninit<Box<[V]>>>; DEFAULT_BLOCK_MAX]>,
	next_free_slot: AtomicU64,
	block_size_log: u32,
	blocks_allocated: AtomicU16,
	instance_id: u16,
}

unsafe impl<V: Default> Send for AtomicSlots<V> {}
unsafe impl<V: Default> Sync for AtomicSlots<V> {}

#[derive(Debug)]
pub enum AtomicSlotsErr {
	OutOfSlots,
}

const BLOCKS_ALLOCATED_COUNT_MASK: u16 = 0x7FFF;
const BLOCKS_ALLOCATED_ALLOCATING_FLAG: u16 = 0x8000;
const_assert_eq!(BLOCKS_ALLOCATED_COUNT_MASK & BLOCKS_ALLOCATED_ALLOCATING_FLAG, 0);

// FIXME: verify all orderings, add concurrency tests, add free method
impl<V: Default> AtomicSlots<V> {
	/// Creates a new [`AtomicSlots`] instance.
	/// `block_size` must be larger than 0 and will be rounded down to the next power of two. It indicates the size of the first block, whereas the next block will always be double the size of the previous one.
	/// `blocks_max` should by default be [`DEFAULT_BLOCK_MAX`], as the internal smallvec has a capacity of that.
	pub fn new(block_size: u32, blocks_max: u16) -> Self {
		let block_size_log = block_size.checked_ilog2().expect("Block size may not be zero");

		#[cfg(not(feature = "loom"))]
		let instance_id = {
			static INSTANCE_COUNTER: AtomicU16 = AtomicU16::new(0);
			INSTANCE_COUNTER.fetch_add(1, Relaxed)
		};
		#[cfg(feature = "loom")]
		let instance_id = 0;

		Self {
			blocks: (0..blocks_max)
				.map(|_| UnsafeCell::new(MaybeUninit::uninit()))
				.collect::<SmallVec<_>>(),
			next_free_slot: AtomicU64::new(0),
			block_size_log,
			blocks_allocated: AtomicU16::new(0),
			instance_id,
		}
	}

	pub fn allocate(&self) -> Result<SlotKey, AtomicSlotsErr> {
		// allocate a new slot
		let slot = self.make_key(self.next_free_slot.fetch_add(1, Relaxed));

		// out of slots
		if slot.block >= self.blocks.len() as u16 {
			return Err(AtomicSlotsErr::OutOfSlots);
		}

		let mut spin_wait = SpinWait::new();
		loop {
			// block already allocated, may need to be acquired
			let blocks_allocated = self.blocks_allocated.load(Acquire) & BLOCKS_ALLOCATED_COUNT_MASK;
			if slot.block < blocks_allocated {
				return Ok(slot);
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
					return Ok(slot);
				}
				// spin and retry
				Err(_) => {}
			}

			spin_wait.spin();
		}
	}

	fn make_key(&self, index: u64) -> SlotKey {
		let base_offset = 1u64 << self.block_size_log;
		let shift = (index + base_offset).ilog2();
		let block = shift - self.block_size_log;

		let slot_offset = (1u64 << shift) - base_offset;
		let slot = index - slot_offset;

		SlotKey {
			instance_id: self.instance_id,
			block: block.try_into().unwrap(),
			slot: slot.try_into().unwrap(),
		}
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
		for (block_size, blocks_max) in [(4, 6), (16, 4), (64, 2)] {
			let slots = AtomicSlots::<u32>::new(block_size, blocks_max);
			let instance_id = slots.instance_id;

			let index_counter = AtomicU64::new(0);
			for block in 0..blocks_max {
				for slot in 0..(block_size << block) {
					assert_eq!(
						slots.make_key(index_counter.fetch_add(1, Relaxed)),
						SlotKey {
							instance_id,
							block,
							slot
						}
					)
				}
			}
		}
	}

	#[test]
	#[should_panic(expected = "Block size may not be zero")]
	fn test_zero_block_size() {
		AtomicSlots::<u32>::new(0, 1);
	}

	#[test]
	#[should_panic(expected = "SlotIndex used with wrong AtomicSlots instance!")]
	fn test_wrong_atomic_slots() {
		let slots1 = AtomicSlots::<u32>::new(4, 1);
		let slots2 = AtomicSlots::<u32>::new(4, 1);
		let slot = slots1.allocate().unwrap();
		slots2[slot];
	}

	#[test]
	fn test_alloc_within_block() {
		let mut slots = AtomicSlots::<u32>::new(16, 1);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		assert_eq!(slots.allocate().unwrap(), slots.make_key(0));
		assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		for i in 0..5 {
			assert_eq!(slots.allocate().unwrap(), slots.make_key(i + 1));
		}

		unsafe {
			assert_eq!(slots.blocks.len(), 1);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
			assert_eq!(slots.blocks[0].get_mut().assume_init_ref().len(), 16);
		}
	}

	#[test]
	fn test_alloc_new_block() {
		unsafe {
			let mut slots = AtomicSlots::<u32>::new(4, 3);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
			for i in 0..4 {
				assert_eq!(slots.allocate().unwrap(), slots.make_key(i));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
			}
			assert_eq!(slots.blocks[0].get_mut().assume_init_ref().len(), 4);

			println!("new block!");
			for i in 0..8 {
				assert_eq!(slots.allocate().unwrap(), slots.make_key(i + 4));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 2);
			}
			assert_eq!(slots.blocks[1].get_mut().assume_init_ref().len(), 8);

			println!("new block 2!");
			for i in 0..16 {
				assert_eq!(slots.allocate().unwrap(), slots.make_key(i + 12));
				assert_eq!(slots.blocks_allocated.load(Relaxed), 3);
			}
			assert_eq!(slots.blocks[2].get_mut().assume_init_ref().len(), 16);
		}
	}

	#[test]
	fn test_alloc_out_of_slots() {
		let slots = AtomicSlots::<u32>::new(4, 1);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		for i in 0..4 {
			assert_eq!(slots.allocate().unwrap(), slots.make_key(i));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		}
		let err_slot = slots.allocate();
		assert!(
			matches!(err_slot, Err(AtomicSlotsErr::OutOfSlots)),
			"slot: {:?}",
			err_slot
		);
	}

	#[test]
	fn test_block_size_not_pow_2() {
		let slots = AtomicSlots::<u32>::new(14, 1);
		assert_eq!(1 << slots.block_size_log, 8);
	}

	#[test]
	fn test_alloc_block_sizes() {
		for (block_size, blocks_max) in [(4, 6), (16, 4), (64, 2)] {
			let mut slots = AtomicSlots::<u32>::new(block_size, blocks_max);
			while let Ok(_) = slots.allocate() {}

			unsafe {
				assert_eq!(slots.blocks.len(), blocks_max as usize);
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
			let slots = Arc::new(AtomicSlots::<u32>::new(4, 16));
			let slots2 = slots.clone();
			crate::sync::thread::spawn(move || {
				slots2.allocate().unwrap();
			});
			slots.allocate().unwrap();
			slots.allocate().unwrap();
		})
	}

	#[test]
	fn loom_alloc_next_block() {
		loom::model(|| {
			let block_size = 4;
			let slots = Arc::new(AtomicSlots::<u32>::new(block_size, 16));
			for _ in 0..block_size {
				slots.allocate().unwrap();
			}

			let slots2 = slots.clone();
			crate::sync::thread::spawn(move || {
				slots2.allocate().unwrap();
			});
			slots.allocate().unwrap();
		})
	}

	#[test]
	fn loom_access() {
		loom::model(|| {
			let slots = Arc::new(AtomicSlots::<u32>::new(4, 16));
			{
				let slots = slots.clone();
				crate::sync::thread::spawn(move || {
					let key = slots.allocate().unwrap();
					assert_eq!(slots.with(key, |slot| *slot), 0);
				});
			}
			let key = slots.allocate().unwrap();
			assert_eq!(slots.with(key, |slot| *slot), 0);
		})
	}
}
