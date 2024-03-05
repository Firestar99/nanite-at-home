use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::mem::MaybeUninit;
use std::ops::Index;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64};
use std::sync::atomic::Ordering::{Relaxed, Release};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;
use smallvec::SmallVec;
use static_assertions::assert_eq_size;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SlotIndex {
	instance_id: u16,
	block: u16,
	slot: u32,
}

assert_eq_size!(SlotIndex, u64);

/// A concurrent SlotMap that does not reallocate its slots.
pub struct AtomicSlots<V: Default> {
	state: AtomicU32,
	block_size_log: u32,
	next_free_slot: AtomicU64,
	instance_id: u16,
	blocks_allocated: AtomicU16,
	blocks: SmallVec<[UnsafeCell<MaybeUninit<Box<[V]>>>; 64]>,
	free_slots: Mutex<Vec<SlotIndex>>,
}

#[derive(Debug)]
pub enum AtomicSlotsErr {
	OutOfSlots,
}

#[derive(Copy, Clone, FromPrimitive, ToPrimitive)]
#[repr(u32)]
enum State {
	NoFreeSlots,
	HasFreeSlots,
	AllocatingNewBlockLock,
}

// FIXME: verify all orderings, add concurrency tests, add free method
impl<V: Default> AtomicSlots<V> {
	pub fn new(block_size: u32, blocks_max: u16) -> Self {
		let block_size_log = block_size.checked_ilog2().expect("Block size may not be zero");
		static INSTANCE_COUNTER: AtomicU16 = AtomicU16::new(0);
		Self {
			state: AtomicU32::new(State::NoFreeSlots as u32),
			block_size_log,
			blocks_allocated: AtomicU16::new(0),
			blocks: (0..blocks_max)
				.map(|_| UnsafeCell::new(MaybeUninit::uninit()))
				.collect::<SmallVec<_>>(),
			free_slots: Mutex::new(Vec::new()),
			next_free_slot: AtomicU64::new(0),
			instance_id: INSTANCE_COUNTER.fetch_add(1, Relaxed),
		}
	}

	pub fn allocate(&self) -> Result<SlotIndex, AtomicSlotsErr> {
		let mut old = State::from_u32(self.state.load(Relaxed)).unwrap();
		loop {
			match old {
				State::HasFreeSlots => {
					// try to get a free slot
					if let Some(slot) = self.free_slots.lock().pop() {
						return Ok(slot);
					} else {
						// no free slots left -> try to update to NoFreeSlots
						old = self
							.state
							.compare_exchange_weak(old as u32, State::NoFreeSlots as u32, Relaxed, Relaxed)
							// success: jump to NoFreeSlots
							.map(|_| State::NoFreeSlots)
							// err: retry
							.unwrap_or_else(|err| State::from_u32(err).unwrap());
					}
				}
				State::NoFreeSlots => {
					// allocate a new slot
					let slot = self.get_index(self.next_free_slot.fetch_add(1, Relaxed));

					loop {
						// SlotBlock already allocated
						if slot.block < self.blocks_allocated.load(Relaxed) {
							return Ok(slot);
						}

						// out of slots
						if slot.block >= self.blocks.len() as u16 {
							return Err(AtomicSlotsErr::OutOfSlots);
						}

						// must allocate a new memory block
						match self.state.compare_exchange_weak(
							old as u32,
							State::AllocatingNewBlockLock as u32,
							Relaxed,
							Relaxed,
						) {
							Ok(_) => {
								// SAFETY: the struct is locked to ensure we are the only ones allocating, and indices larger than blocks_allocated should never be
								// accessed anywhere before this has passed
								unsafe {
									(&mut *self.blocks.get(slot.block as usize).unwrap().get()).write(
										(0..(1 << (slot.block as u32 + self.block_size_log)))
											.map(|_| V::default())
											.collect::<Vec<_>>()
											.into_boxed_slice(),
									);
								}
								self.blocks_allocated.store(slot.block + 1, Release);
								self.state.store(old as u32, Release);
								return Ok(slot);
							}
							Err(err) => old = State::from_u32(err).unwrap(),
						}

						spin_loop();
					}
				}
				State::AllocatingNewBlockLock => {
					spin_loop();
				}
			}
		}
	}

	fn get_index(&self, index: u64) -> SlotIndex {
		let base_offset = 1u64 << self.block_size_log;
		let shift = (index + base_offset).ilog2();
		let block = shift - self.block_size_log;

		let slot_offset = (1u64 << shift) - base_offset;
		let slot = index - slot_offset;

		SlotIndex {
			instance_id: self.instance_id,
			block: block.try_into().unwrap(),
			slot: slot.try_into().unwrap(),
		}
	}
}

impl<V: Default> Index<SlotIndex> for AtomicSlots<V> {
	type Output = V;

	fn index(&self, slot: SlotIndex) -> &Self::Output {
		assert_eq!(
			slot.instance_id, self.instance_id,
			"SlotIndex used with wrong AtomicSlots instance!"
		);
		unsafe { &(&mut *self.blocks[slot.block as usize].get()).assume_init_ref()[slot.slot as usize] }
	}
}

impl<V: Default> Drop for AtomicSlots<V> {
	fn drop(&mut self) {
		// SAFETY: `self.blocks` has at least as many entries populated as `self.blocks_allocated`
		// (though could be more in case of a panic during allocation)
		unsafe {
			for i in 0..*self.blocks_allocated.get_mut() as usize {
				self.blocks[i].get_mut().assume_init_drop();
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_get_index() {
		for (block_size, blocks_max) in [(4, 6), (16, 4), (64, 2)] {
			let slots = AtomicSlots::<u32>::new(block_size, blocks_max);
			let instance_id = slots.instance_id;

			let index_counter = AtomicU64::new(0);
			for block in 0..blocks_max {
				for slot in 0..(block_size << block) {
					assert_eq!(
						slots.get_index(index_counter.fetch_add(1, Relaxed)),
						SlotIndex {
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
	#[should_panic("Block size may not be zero")]
	fn test_zero_block_size() {
		AtomicSlots::<u32>::new(0, 1);
	}

	#[test]
	#[should_panic("SlotIndex used with wrong AtomicSlots instance!")]
	fn test_wrong_atomic_slots() {
		let slots1 = AtomicSlots::<u32>::new(4, 1);
		let slots2 = AtomicSlots::<u32>::new(4, 1);
		let slot = slots1.allocate().unwrap();
		slots2[slot];
	}

	#[test]
	fn test_alloc_within_block() {
		let slots = AtomicSlots::<u32>::new(16, 1);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		assert_eq!(slots.allocate().unwrap(), slots.get_index(0));
		assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		for i in 0..5 {
			assert_eq!(slots.allocate().unwrap(), slots.get_index(i + 1));
		}
	}

	#[test]
	fn test_alloc_new_block() {
		let slots = AtomicSlots::<u32>::new(4, 3);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		for i in 0..4 {
			assert_eq!(slots.allocate().unwrap(), slots.get_index(i));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		}

		println!("new block!");
		for i in 0..8 {
			assert_eq!(slots.allocate().unwrap(), slots.get_index(i + 4));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 2);
		}

		println!("new block 2!");
		for i in 0..16 {
			assert_eq!(slots.allocate().unwrap(), slots.get_index(i + 12));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 3);
		}
	}

	#[test]
	fn test_alloc_out_of_slots() {
		let slots = AtomicSlots::<u32>::new(4, 1);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		for i in 0..4 {
			assert_eq!(slots.allocate().unwrap(), slots.get_index(i));
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		}
		let err_slot = slots.allocate();
		assert!(
			matches!(err_slot, Err(AtomicSlotsErr::OutOfSlots)),
			"slot: {:?}",
			err_slot
		);
	}
}
