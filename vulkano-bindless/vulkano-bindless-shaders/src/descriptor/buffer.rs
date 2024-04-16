use crate::descriptor::descriptor_type::private;
use crate::descriptor::descriptors::Descriptors;
use crate::descriptor::{DescType, ResourceTable, ValidDesc};
use core::marker::PhantomData;
use core::ops::IndexMut;
use spirv_std::ByteAddressableBuffer;

pub struct Buffer<T: ?Sized> {
	_phantom: PhantomData<T>,
}

pub struct BufferTable;

impl<T: ?Sized> private::SealedTrait for Buffer<T> {}

impl private::SealedTrait for BufferTable {}

impl<T: ?Sized> DescType for Buffer<T> {
	type ResourceTable = BufferTable;
	type AccessType<'a> = BufferSlice<'a, 'a, T>;
}

impl ResourceTable for BufferTable {
	const BINDING: u32 = 0;
}

pub trait BufferAccess<T: ?Sized> {
	fn access<'a>(&'a self, descriptors: &'a mut Descriptors<'a>) -> BufferSlice<'a, 'a, T>;
}

impl<T: ?Sized, A: ValidDesc<Buffer<T>>> BufferAccess<T> for A {
	fn access<'a>(&'a self, descriptors: &'a mut Descriptors<'a>) -> BufferSlice<'a, 'a, T> {
		BufferSlice::new(descriptors, self)
	}
}

pub struct BufferSlice<'a, 'b, T: ?Sized> {
	descriptors: &'a mut Descriptors<'b>,
	id: u32,
	_phantom: PhantomData<T>,
}

impl<'a, 'b, T: ?Sized> BufferSlice<'a, 'b, T> {
	pub fn new(descriptors: &'a mut Descriptors<'b>, desc: &impl ValidDesc<Buffer<T>>) -> Self {
		Self {
			descriptors,
			id: desc.id(),
			_phantom: PhantomData {},
		}
	}

	fn byte_buffer(&mut self) -> ByteAddressableBuffer {
		ByteAddressableBuffer::new(self.descriptors.buffer_data.index_mut(self.id as usize))
	}
}

impl<'a, 'b, T: Sized> BufferSlice<'a, 'b, T> {
	pub fn load(mut self) -> T {
		unsafe { self.byte_buffer().load(0) }
	}
}

impl<'a, 'b, T> BufferSlice<'a, 'b, [T]> {
	pub fn load(mut self, offset: usize) -> T {
		unsafe { self.byte_buffer().load(offset as u32) }
	}

	pub fn load_unchecked(mut self, offset: usize) -> T {
		unsafe { self.byte_buffer().load_unchecked(offset as u32) }
	}
}
