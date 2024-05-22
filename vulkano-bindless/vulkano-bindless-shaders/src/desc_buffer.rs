use bytemuck::AnyBitPattern;

pub unsafe trait DescBuffer: Copy + Clone + Send + Sync {
	type DescStatic: DescBuffer + AnyBitPattern + Send + Sync;

	/// Unsafely transmute TransientDesc lifetime to static
	///
	/// # Safety
	/// Must only be used just before writing push constants
	unsafe fn to_static_desc(&self) -> Self::DescStatic;
}

mod desc_buffer_native {
	use super::*;
	use core::marker::PhantomPinned;

	trait DescBufferNative: AnyBitPattern + Send + Sync {}

	unsafe impl<T> DescBuffer for T
	where
		T: DescBufferNative,
	{
		type DescStatic = T;

		unsafe fn to_static_desc(&self) -> Self::DescStatic {
			*self
		}
	}

	impl DescBufferNative for () {}
	impl DescBufferNative for u8 {}
	impl DescBufferNative for i8 {}
	impl DescBufferNative for u16 {}
	impl DescBufferNative for i16 {}
	impl DescBufferNative for u32 {}
	impl DescBufferNative for i32 {}
	impl DescBufferNative for u64 {}
	impl DescBufferNative for i64 {}
	impl DescBufferNative for usize {}
	impl DescBufferNative for isize {}
	impl DescBufferNative for u128 {}
	impl DescBufferNative for i128 {}
	impl DescBufferNative for f32 {}
	impl DescBufferNative for f64 {}
	// impl<T: DescBufferNative> DescBufferNative for Wrapping<T> {}
	// impl<T: ?Sized + 'static> DescBufferNative for PhantomData<T> {}
	impl DescBufferNative for PhantomPinned {}
	// impl<T: DescBufferNative> DescBufferNative for ManuallyDrop<T> {}
}
