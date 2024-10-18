use core::marker::PhantomData;
use core::mem::size_of;
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use spirv_std::arch::{
	atomic_i_add, subgroup_ballot, subgroup_ballot_bit_count, subgroup_ballot_exclusive_bit_count,
	subgroup_broadcast_first, subgroup_elect,
};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::ByteAddressableBuffer;

pub struct AllocationBufferWriter<'a, T: BufferContent> {
	buffer: &'a mut [u32],
	atomic_counter: &'a mut u32,
	_phantom: PhantomData<T>,
}

impl<'a, T: BufferContent> AllocationBufferWriter<'a, T> {
	/// Creates a new `AllocationBufferWriter` from a buffer and an atomic u32, that allows writing values into the
	/// buffer concurrently from multiple invocations and workgroups. The values are written in a compacted way, so
	/// there are no "holes" of unused indices in the resulting buffer.
	///
	/// # Safety
	/// The `T` generic must be the same for all users of this `buffer` and must have an alignment of at least 4.
	/// The `atomic_counter` must correspond to this `buffer` and the `buffer` must not be read (or be used to construct
	/// an [`AllocationBufferReader`]) until an appropriate memory barrier has occurred.
	pub unsafe fn new(buffer: &'a mut [u32], atomic_counter: &'a mut u32) -> Self {
		Self {
			buffer,
			atomic_counter,
			_phantom: PhantomData {},
		}
	}

	/// Allocates space and writes T's to the buffer. One may call this function from non-uniform flow control, then
	/// only the active invocations will write T's. Returns true if successful, false if the buffer ran out of capacity.
	///
	/// Uses subgroup intrinsics to efficiently allocate space with just a single atomic operation per subgroup.
	#[must_use]
	pub fn subgroup_write_non_uniform(&mut self, t: T) -> bool {
		let index = unsafe {
			let ballot = subgroup_ballot(true);
			let count = subgroup_ballot_bit_count(ballot);
			let base_index = if subgroup_elect() {
				atomic_i_add::<_, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(self.atomic_counter, count)
			} else {
				0
			};
			let base_index = subgroup_broadcast_first(base_index);
			let inv_index = subgroup_ballot_exclusive_bit_count(ballot);
			base_index + inv_index
		};

		let sizeof = size_of::<T>() as u32;
		let byte_index = index * sizeof;
		if byte_index + sizeof > self.buffer.len() as u32 {
			false
		} else {
			unsafe {
				ByteAddressableBuffer::from_mut_slice(self.buffer).store(byte_index, t);
			}
			true
		}
	}
}

pub struct AllocationBufferReader<'a, T: BufferContent> {
	buffer: &'a [u32],
	len: u32,
	_phantom: PhantomData<T>,
}

impl<'a, T: BufferContent> AllocationBufferReader<'a, T> {
	/// Creates a new `AllocationBufferReader` to read values written by a previous [`AllocationBufferWriter`]. See the
	/// writer for details.
	///
	/// # Safety
	/// The `T` generic must be the same for all users of this `buffer` and must have an alignment of at least 4.
	/// The `atomic_counter` must correspond to this `buffer` and the `buffer` must not be written simultaneously.
	pub unsafe fn new(buffer: &'a [u32], atomic_counter: &'a u32) -> Self {
		// plain load is fine, safety in writer guarantees a barrier
		let len = *atomic_counter;
		Self {
			buffer,
			len,
			_phantom: PhantomData {},
		}
	}

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
				ByteAddressableBuffer::from_slice(self.buffer).load_unchecked(index * size_of::<T>() as u32)
			} else {
				// len must not be referred to as self.len but as a local variable, rust-gpu doesn't like it otherwise
				panic!("index out of bounds: the len is {} but the index is {}", len, index);
			}
		}
	}

	pub unsafe fn read_unchecked(&self, index: u32) -> T {
		unsafe { ByteAddressableBuffer::from_slice(self.buffer).load_unchecked(index * size_of::<T>() as u32) }
	}
}
