use crate::BufferStructIdentity;
use crate::BufferStructPlain;
use bytemuck::Pod;
use core::marker::PhantomData;
use core::num::Wrapping;
use spirv_std::arch::IndexUnchecked;

macro_rules! identity {
	($t:ty) => {
		unsafe impl BufferStructIdentity for $t {}
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

identity!(spirv_std::memory::Semantics);
identity!(spirv_std::ray_tracing::RayFlags);
identity!(spirv_std::indirect_command::DrawIndirectCommand);
identity!(spirv_std::indirect_command::DrawIndexedIndirectCommand);
identity!(spirv_std::indirect_command::DispatchIndirectCommand);
identity!(spirv_std::indirect_command::DrawMeshTasksIndirectCommandEXT);
identity!(spirv_std::indirect_command::TraceRaysIndirectCommandKHR);
// not pod
// identity!(spirv_std::indirect_command::TraceRaysIndirectCommand2KHR);

unsafe impl<T: BufferStructPlain> BufferStructPlain for Wrapping<T>
where
	// unfortunately has to be Pod, even though AnyBitPattern would be sufficient,
	// due to bytemuck doing `impl<T: Pod> AnyBitPattern for T {}`
	// see https://github.com/Lokathor/bytemuck/issues/164
	T::Transfer: Pod,
{
	type Transfer = Wrapping<T::Transfer>;

	#[inline]
	unsafe fn write(self) -> Self::Transfer {
		unsafe { Wrapping(T::write(self.0)) }
	}

	#[inline]
	unsafe fn read(from: Self::Transfer) -> Self {
		unsafe { Wrapping(T::read(from.0)) }
	}
}

unsafe impl<T: BufferStructPlain + 'static> BufferStructPlain for PhantomData<T> {
	type Transfer = PhantomData<T>;

	#[inline]
	unsafe fn write(self) -> Self::Transfer {
		PhantomData {}
	}

	#[inline]
	unsafe fn read(_from: Self::Transfer) -> Self {
		PhantomData {}
	}
}

/// Potential problem: you can't impl this for an array of BufferStruct, as it'll conflict with this impl due to the
/// blanket impl on all BufferStructPlain types. If this becomes a problem, we could fix it by creating a separate
/// `BufferStructPlainAutoDerive: BufferStructPlain` type and have only it get the blanket impl, allowing us to specify
/// an impl using `BufferStruct` doing the exact same thing.
unsafe impl<T: BufferStructPlain, const N: usize> BufferStructPlain for [T; N]
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
	unsafe fn write(self) -> Self::Transfer {
		unsafe {
			let mut ret = [T::Transfer::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::write(*self.index_unchecked(i));
			}
			ret
		}
	}

	#[inline]
	unsafe fn read(from: Self::Transfer) -> Self {
		unsafe {
			let mut ret = [T::default(); N];
			for i in 0..N {
				*ret.index_unchecked_mut(i) = T::read(*from.index_unchecked(i));
			}
			ret
		}
	}
}
