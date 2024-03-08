use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::mem::MaybeUninit;
use std::ops::Index;
use std::sync::atomic::Ordering::{Relaxed, Release};
use std::sync::atomic::{AtomicU32, AtomicUsize};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use parking_lot::Mutex;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SlotIndex(usize);

/// A concurrent SlotMap
pub struct AtomicSlots<V> {
	state: AtomicU32,
	block_size_log: u32,
	blocks_allocated: AtomicU32,
	blocks: Box<[UnsafeCell<MaybeUninit<Box<[MaybeUninit<V>]>>>]>,
	free_slots: Mutex<Vec<SlotIndex>>,
	next_free_slot: AtomicUsize,
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

impl<V> AtomicSlots<V> {
	pub fn new(block_size: usize, blocks_max: usize) -> Self {
		let block_size_log = block_size.checked_ilog2().expect("Block size may not be zero");
		Self {
			state: AtomicU32::new(State::NoFreeSlots as u32),
			block_size_log,
			blocks_allocated: AtomicU32::new(0),
			blocks: (0..blocks_max)
				.map(|_| UnsafeCell::new(MaybeUninit::uninit()))
				.collect::<Vec<_>>()
				.into_boxed_slice(),
			free_slots: Mutex::new(Vec::new()),
			next_free_slot: AtomicUsize::new(0),
		}
	}

	pub fn allocate(&self, default: V) -> Result<SlotIndex, AtomicSlotsErr> {
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
					let slot = SlotIndex(self.next_free_slot.fetch_add(1, Relaxed));
					let index = self.get_index(slot);

					loop {
						if index.0 < self.blocks_allocated.load(Relaxed) {
							// SlotBlock already allocated
							return Ok(slot);
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
									(&mut *self.blocks.get(index.0 as usize).unwrap().get()).write(
										(0..(1 << (index.0 + self.block_size_log)))
											.map(|_| MaybeUninit::uninit())
											.collect::<Vec<_>>()
											.into_boxed_slice(),
									);
								}
								self.blocks_allocated.store(index.0, Release);
								self.state.store(old as u32, Release);
								return Ok(slot);
							}
							// FIXME: AllocatingNewBlockLock set by another thread must park thread!
							Err(err) => old = State::from_u32(err).unwrap(),
						}
					}
				}
				State::AllocatingNewBlockLock => {
					spin_loop();
				}
			}
		}
	}

	fn get_index(&self, slot: SlotIndex) -> (u32, usize) {
		// slot.0.leading_zeros()
		// let first = slot.0.ilog2() - self.block_size_log;
		let first = slot
			.0
			.checked_ilog2()
			.and_then(|x| x.checked_sub(self.block_size_log))
			.unwrap_or(0);
		let second = slot.0 & ((1 << first) - 1);
		(first, second)
	}
}

impl<V> Index<SlotIndex> for AtomicSlots<V> {
	type Output = V;

	fn index(&self, slot: SlotIndex) -> &Self::Output {
		let index = self.get_index(slot);

		unsafe { (&mut *self.blocks[index.0 as usize].get()).assume_init_ref()[index.1].assume_init_ref() }
	}
}

impl<V> Drop for AtomicSlots<V> {
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
	#[should_panic]
	fn test_zero_block_size() {
		AtomicSlots::<u32>::new(0, 1);
	}

	#[test]
	fn test_alloc_within_block() {
		let slots = AtomicSlots::<u32>::new(16, 1);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		assert_eq!(slots.allocate().unwrap().0, 0);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		for i in 0..5 {
			assert_eq!(slots.allocate().unwrap().0, i + 1);
		}
	}

	#[test]
	fn test_alloc_new_block() {
		let slots = AtomicSlots::<u32>::new(4, 2);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		for i in 0..4 {
			assert_eq!(slots.allocate().unwrap().0, i);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 1);
		}

		println!("new block!");
		for i in 0..4 {
			assert_eq!(slots.allocate().unwrap().0, 4 + i);
			assert_eq!(slots.blocks_allocated.load(Relaxed), 2);
		}
	}

	#[test]
	fn test_alloc_out_of_slots() {
		let slots = AtomicSlots::<u32>::new(4, 2);
		assert_eq!(slots.blocks_allocated.load(Relaxed), 0);
		for i in 0..8 {
			assert_eq!(slots.allocate().unwrap().0, i);
			assert_eq!(slots.blocks_allocated.load(Relaxed) as usize, i / 4 + 1);
		}
		assert!(matches!(slots.allocate(), Err(AtomicSlotsErr::OutOfSlots)));
	}
}
