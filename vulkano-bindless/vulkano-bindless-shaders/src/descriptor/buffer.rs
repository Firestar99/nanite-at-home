use crate::descriptor::descriptor_type::private;
use crate::descriptor::descriptors::Descriptors;
use crate::descriptor::{DescType, ResourceTable, ValidDesc};
use core::marker::PhantomData;
use core::mem;
use spirv_std::byte_addressable_buffer::buffer_load_intrinsic;

pub struct Buffer<T: ?Sized> {
	_phantom: PhantomData<T>,
}

pub struct BufferTable;

impl<T: ?Sized> private::SealedTrait for Buffer<T> {}

impl private::SealedTrait for BufferTable {}

impl<T: ?Sized> DescType for Buffer<T> {
	type ResourceTable = BufferTable;
	type AccessType<'a> = BufferSlice<'a, T>;
}

impl ResourceTable for BufferTable {
	const BINDING: u32 = 0;
}

pub trait BufferAccess<T: ?Sized> {
	fn access<'a>(&'a self, descriptors: &'a Descriptors<'a>) -> BufferSlice<'a, T>;
}

impl<T: ?Sized, A: ValidDesc<Buffer<T>>> BufferAccess<T> for A {
	fn access<'a>(&'a self, descriptors: &'a Descriptors<'a>) -> BufferSlice<'a, T> {
		BufferSlice::new(descriptors, self)
	}
}

pub struct BufferSlice<'a, T: ?Sized> {
	buffer: &'a [u32],
	_phantom: PhantomData<T>,
}

impl<'a, T: ?Sized> BufferSlice<'a, T> {
	pub fn new(descriptors: &'a Descriptors<'a>, desc: &impl ValidDesc<Buffer<T>>) -> Self {
		Self {
			buffer: unsafe { descriptors.buffer_data.index(desc.id() as usize) },
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
	pub fn load(&self, byte_index: usize) -> T {
		let size = mem::size_of::<T>();
		let len = self.buffer.len() * 4;
		if byte_index + size > len {
			// FIXME why does this debug printf mispile and len disappears?
			panic!("Index out of range: {} {} + {} > {}", len, byte_index, size, len);
		}
		unsafe { self.load_unchecked(byte_index) }
	}

	/// Loads a T at an `byte_index` offset from the buffer. `byte_index` must be a multiple of 4, otherwise,
	/// it will get silently rounded down to the nearest multiple of 4.
	///
	/// # Safety
	/// `byte_index` must be in bounds of the buffer
	pub unsafe fn load_unchecked(&self, byte_index: usize) -> T {
		unsafe { buffer_load_intrinsic(self.buffer, byte_index as u32) }
	}
}
