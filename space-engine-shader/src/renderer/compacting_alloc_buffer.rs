use rust_gpu_bindless_macros::BufferStruct;
use rust_gpu_bindless_shaders::buffer_content::BufferStructPlain;
use rust_gpu_bindless_shaders::descriptor::{Buffer, BufferSlice, Descriptors, MutBuffer, TransientDesc};
use spirv_std::arch::{
	atomic_i_add, subgroup_ballot, subgroup_ballot_bit_count, subgroup_ballot_exclusive_bit_count,
	subgroup_broadcast_first, subgroup_elect,
};
use spirv_std::memory::{Scope, Semantics};

#[derive(Copy, Clone, BufferStruct)]
pub struct CompactingAllocBufferWriter<'a, T: BufferStructPlain> {
	pub buffer: TransientDesc<'a, MutBuffer<[T]>>,
	pub indirect_args: TransientDesc<'a, MutBuffer<[u32; 3]>>,
}

impl<'a, T: BufferStructPlain> CompactingAllocBufferWriter<'a, T> {
	/// Allocates space and writes T's to the buffer. One may call this function from non-uniform flow control, then
	/// only the active invocations will write T's. Returns true if successful, false if the buffer ran out of capacity.
	///
	/// Uses subgroup intrinsics to efficiently allocate space with just a single atomic operation per subgroup.
	#[must_use]
	pub fn allocate<'b>(&'b self, descriptors: &mut Descriptors) -> Allocation<'b, T> {
		let index = unsafe {
			let ballot = subgroup_ballot(true);
			let count = subgroup_ballot_bit_count(ballot);
			let base_index = if subgroup_elect() {
				let atomic_counter = &mut self.indirect_args.access(descriptors).into_raw_mut()[0];
				atomic_i_add::<_, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(atomic_counter, count)
			} else {
				0
			};
			let base_index = subgroup_broadcast_first(base_index);
			let inv_index = subgroup_ballot_exclusive_bit_count(ballot);
			base_index + inv_index
		} as usize;
		Allocation { writer: self, index }
	}
}

pub struct Allocation<'a, T: BufferStructPlain> {
	writer: &'a CompactingAllocBufferWriter<'a, T>,
	index: usize,
}

impl<'a, T: BufferStructPlain> Allocation<'a, T> {
	pub fn write(self, descriptors: &mut Descriptors, t: T) -> bool {
		let mut buffer = self.writer.buffer.access(descriptors);
		if self.index < buffer.len() {
			unsafe { buffer.store(self.index, t) };
			true
		} else {
			false
		}
	}
}

#[derive(Copy, Clone, BufferStruct)]
pub struct CompactingAllocBufferReader<'a, T: BufferStructPlain> {
	pub buffer: TransientDesc<'a, Buffer<[T]>>,
	pub indirect_args: TransientDesc<'a, Buffer<[u32; 3]>>,
}

impl<'a, T: BufferStructPlain> CompactingAllocBufferReader<'a, T> {
	pub fn access<'b>(&self, descriptors: &'b Descriptors) -> CompactingAllocBufferReaderAccessed<'b, T> {
		let slice = self.buffer.access(descriptors);
		CompactingAllocBufferReaderAccessed {
			buffer: slice,
			len: self.indirect_args.access(descriptors).load()[0],
		}
	}
}

pub struct CompactingAllocBufferReaderAccessed<'a, T: BufferStructPlain> {
	pub buffer: BufferSlice<'a, [T]>,
	pub len: u32,
}

impl<'a, T: BufferStructPlain> CompactingAllocBufferReaderAccessed<'a, T> {
	pub fn len(&self) -> u32 {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn read(&self, index: u32) -> T {
		unsafe {
			let len = self.len;
			if index < len {
				self.read_unchecked(index)
			} else {
				// len must not be referred to as self.len but as a local variable, rust-gpu doesn't like it otherwise
				panic!("index out of bounds: the len is {} but the index is {}", len, index);
			}
		}
	}

	pub unsafe fn read_unchecked(&self, index: u32) -> T {
		self.buffer.load_unchecked(index as usize)
	}
}
