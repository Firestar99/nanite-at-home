use crate::buffer_content::{BufferContent, BufferStruct, Metadata};
use crate::descriptor::descriptor_content::DescContent;
use core::marker::PhantomData;
use core::mem;
use spirv_std::ByteAddressableBuffer;

pub struct Buffer<T: BufferContent + ?Sized> {
	_phantom: PhantomData<T>,
}

impl<T: BufferContent + ?Sized> DescContent for Buffer<T> {
	type AccessType<'a> = BufferSlice<'a, T>;
}

pub struct BufferSlice<'a, T: ?Sized> {
	buffer: &'a [u32],
	meta: Metadata,
	_phantom: PhantomData<T>,
}

impl<'a, T: BufferContent + ?Sized> BufferSlice<'a, T> {
	/// # Safety
	/// T needs to match the contents of the buffer
	#[inline]
	pub unsafe fn new(buffer: &'a [u32], meta: Metadata) -> Self {
		Self {
			buffer,
			meta,
			_phantom: PhantomData {},
		}
	}
}

impl<'a, T: BufferStruct> BufferSlice<'a, T> {
	/// Loads a T from the buffer.
	pub fn load(&self) -> T {
		unsafe { T::read(buffer_load_intrinsic::<T::Transfer>(self.buffer, 0), self.meta) }
	}
}

impl<'a, T: BufferStruct> BufferSlice<'a, [T]> {
	/// Loads a T at an `index` offset from the buffer.
	pub fn load(&self, index: usize) -> T {
		let size = mem::size_of::<T::Transfer>();
		let byte_offset = index * size;
		let len = self.buffer.len() * 4;
		if byte_offset + size <= len {
			unsafe {
				T::read(
					buffer_load_intrinsic::<T::Transfer>(self.buffer, byte_offset as u32),
					self.meta,
				)
			}
		} else {
			let len = len / size;
			// TODO mispile: len and index are often wrong
			panic!("index out of bounds: the len is {} but the index is {}", len, index);
		}
	}

	/// Loads a T at an `index` offset from the buffer.
	///
	/// # Safety
	/// `byte_index` must be in bounds of the buffer
	pub unsafe fn load_unchecked(&self, index: usize) -> T {
		unsafe {
			let byte_offset = (index * mem::size_of::<T::Transfer>()) as u32;
			T::read(
				buffer_load_intrinsic::<T::Transfer>(self.buffer, byte_offset),
				self.meta,
			)
		}
	}
}

impl<'a, T: BufferContent + ?Sized> BufferSlice<'a, T> {
	/// Loads an arbitrary type E at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4,
	/// otherwise, it will get silently rounded down to the nearest multiple of 4.
	///
	/// # Safety
	/// E must be a valid arbitrary AnyBitPattern type
	pub unsafe fn load_at_offset<E: BufferStruct>(&self, byte_offset: usize) -> E {
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
	pub unsafe fn load_at_offset_unchecked<E: BufferStruct>(&self, byte_offset: usize) -> E {
		unsafe {
			E::read(
				buffer_load_intrinsic::<E::Transfer>(self.buffer, byte_offset as u32),
				self.meta,
			)
		}
	}
}

unsafe fn buffer_load_intrinsic<T>(buffer: &[u32], offset: u32) -> T {
	ByteAddressableBuffer::from_slice(buffer).load_unchecked(offset)
}
