use crate::buffer_content::{BufferStruct, MetadataCpuInterface};
use crate::descriptor::Metadata;
use bytemuck::Pod;
use core::marker::PhantomData;
use core::num::Wrapping;
use spirv_std::arch::IndexUnchecked;

macro_rules! identity {
	($t:ty) => {
		unsafe impl BufferStruct for $t {
			type Transfer = $t;

			#[inline]
			unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
				self
			}

			#[inline]
			unsafe fn read(from: Self::Transfer, _meta: Metadata) -> Self {
				from
			}
		}
	};
}

identity!(());
identity!(u8);
identity!(u16);
identity!(u32);
identity!(u64);
identity!(u128);
identity!(usize);
identity!(i8);
identity!(i16);
identity!(i32);
identity!(i64);
identity!(i128);
identity!(isize);
identity!(f32);
identity!(f64);

unsafe impl<T: BufferStruct> BufferStruct for Wrapping<T>
where
	// unfortunately has to be Pod, even though AnyBitPattern would be sufficient,
	// due to bytemuck doing `impl<T: Pod> AnyBitPattern for T {}`
	// see https://github.com/Lokathor/bytemuck/issues/164
	T::Transfer: Pod,
{
	type Transfer = Wrapping<T::Transfer>;

	#[inline]
	unsafe fn write_cpu(self, meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		Wrapping(T::write_cpu(self.0, meta))
	}

	#[inline]
	unsafe fn read(from: Self::Transfer, meta: Metadata) -> Self {
		Wrapping(T::read(from.0, meta))
	}
}

unsafe impl<T: BufferStruct + 'static> BufferStruct for PhantomData<T> {
	type Transfer = PhantomData<T>;

	#[inline]
	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		PhantomData {}
	}

	#[inline]
	unsafe fn read(_from: Self::Transfer, _meta: Metadata) -> Self {
		PhantomData {}
	}
}

unsafe impl<T: BufferStruct, const N: usize> BufferStruct for [T; N]
where
	// rust-gpu does not like `[T; N].map()` nor `core::array::from_fn()` nor transmuting arrays with a const generic
	// length, so for now we need to require T: Default and T::Transfer: Default for all arrays.
	T: Default,
	// unfortunately has to be Pod, even though AnyBitPattern would be sufficient,
	// due to bytemuck doing `impl<T: Pod> AnyBitPattern for T {}`
	// see https://github.com/Lokathor/bytemuck/issues/164
	T::Transfer: Pod + Default,
{
	type Transfer = [T::Transfer; N];

	#[inline]
	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		unsafe {
			let mut ret = [T::Transfer::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::write_cpu(*self.index_unchecked(i), _meta);
			}
			ret
		}
	}

	#[inline]
	unsafe fn read(from: Self::Transfer, _meta: Metadata) -> Self {
		unsafe {
			let mut ret = [T::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::read(*from.index_unchecked(i), _meta);
			}
			ret
		}
	}
}
