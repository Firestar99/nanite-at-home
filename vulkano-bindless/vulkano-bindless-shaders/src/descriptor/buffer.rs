use crate::descriptor::descriptor_type::{private, DescType};
use crate::descriptor::descriptors::Descriptors;
use bytemuck::AnyBitPattern;
use core::marker::PhantomData;
use core::mem;
use spirv_std::byte_addressable_buffer::buffer_load_intrinsic;

pub struct Buffer<T: ?Sized + 'static> {
	_phantom: PhantomData<T>,
}

impl<T: ?Sized + 'static> private::SealedTrait for Buffer<T> {}

impl<T: ?Sized + 'static> DescType for Buffer<T> {
	type AccessType<'a> = BufferSlice<'a, T>;

	#[inline]
	fn access<'a>(descriptors: &'a Descriptors<'a>, id: u32) -> Self::AccessType<'a> {
		BufferSlice::new(unsafe { descriptors.buffers.index(id as usize) })
	}
}

pub struct BufferSlice<'a, T: ?Sized> {
	buffer: &'a [u32],
	_phantom: PhantomData<T>,
}

impl<'a, T: ?Sized> BufferSlice<'a, T> {
	#[inline]
	pub fn new(buffer: &'a [u32]) -> Self {
		Self {
			buffer,
			_phantom: PhantomData {},
		}
	}
}

impl<'a, T: Sized> BufferSlice<'a, T> {
	/// Loads a T from the buffer.
	pub fn load(&self) -> T {
		unsafe { buffer_load_intrinsic(self.buffer, 0) }
	}
}

impl<'a, T> BufferSlice<'a, [T]> {
	/// Loads a T at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4, otherwise,
	/// it will get silently rounded down to the nearest multiple of 4.
	pub fn load(&self, index: usize) -> T {
		let size = mem::size_of::<T>();
		let byte_offset = index * size;
		let len = self.buffer.len() * 4;
		if byte_offset + size <= len {
			unsafe { buffer_load_intrinsic(self.buffer, byte_offset as u32) }
		} else {
			let len = len / size;
			// TODO mispile: len and index are often wrong
			panic!("index out of bounds: the len is {} but the index is {}", len, index);
		}
	}

	/// Loads a T at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4, otherwise,
	/// it will get silently rounded down to the nearest multiple of 4.
	///
	/// # Safety
	/// `byte_index` must be in bounds of the buffer
	pub unsafe fn load_unchecked(&self, index: usize) -> T {
		unsafe { buffer_load_intrinsic(self.buffer, (index * mem::size_of::<T>()) as u32) }
	}
}

impl<'a, T: ?Sized> BufferSlice<'a, T> {
	/// Loads an arbitrary type E at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4,
	/// otherwise, it will get silently rounded down to the nearest multiple of 4.
	///
	/// # Safety
	/// E must be a valid arbitrary AnyBitPattern type
	pub unsafe fn load_at_offset<E: AnyBitPattern>(&self, byte_offset: usize) -> E {
		let size = mem::size_of::<E>();
		let len = self.buffer.len() * 4;
		if byte_offset + size <= len {
			unsafe { self.load_at_offset_unchecked(byte_offset) }
		} else {
			// TODO mispile: len and byte_offset are often wrong
			panic!(
				"index out of bounds: the len is {} but the byte_offset is {} + size {}",
				len, byte_offset, size
			);
		}
	}

	/// Loads an arbitrary type E at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4,
	/// otherwise, it will get silently rounded down to the nearest multiple of 4.
	///
	/// # Safety
	/// E must be a valid arbitrary AnyBitPattern type
	/// `byte_index` must be in bounds of the buffer
	pub unsafe fn load_at_offset_unchecked<E: AnyBitPattern>(&self, byte_offset: usize) -> E {
		unsafe { buffer_load_intrinsic(self.buffer, byte_offset as u32) }
	}
}
