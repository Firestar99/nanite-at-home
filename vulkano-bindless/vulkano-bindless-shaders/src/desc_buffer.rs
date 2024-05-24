use crate::descriptor::metadata::Metadata;
use bytemuck::AnyBitPattern;

/// Trait for contents of **buffers** that may contain descriptors requiring conversion.
///
/// See [`DescStruct`]. All [`DescStruct`] also implement [`DescBuffer`] with `TransferDescBuffer = TransferDescStruct`.
///
/// Compared to [`DescStruct`], [`DescBuffer`] also allows for unsized types such as slices to be used. Therefore, it
/// does not offer any conversion functions, since a slice can only be converted element-wise.
///
/// # Safety
/// Should not be manually implemented, see [`DescStruct`].
pub unsafe trait DescBuffer: Send + Sync {
	type TransferDescBuffer: Send + Sync + ?Sized;
}

/// Trait for **sized types** that may contain descriptors requiring conversion and can be stored in a Buffer. Use
/// `#derive[DescBuffer]` on your type to implement this trait.
///
/// The actual type stored in the Buffer is defined by its associated type `TransferDescStruct` and can be converted to
/// and from using [`Self::to_transfer`] and [`Self::from_transfer`]. Types that are [`AnyBitPattern`] automatically
/// implement `DescBuffer` with conversions being identity.
///
/// # Safety
/// Should only be implemented via DescBuffer macro. Only Descriptors may have a manual implementation.
pub unsafe trait DescStruct: Copy + Clone + Sized + Send + Sync {
	type TransferDescStruct: AnyBitPattern + Send + Sync;

	/// Transmute Self into a transferable struct on the CPU that can subsequently be sent to the GPU. This includes
	/// unsafely transmuting [`FrameInFlight`] lifetimes to `'static`, so it's [`AnyBitPattern`]`: 'static` and
	/// can be written to a buffer.
	///
	/// # Safety
	/// Should only be implemented via DescBuffer macro and only used internally by `BindlessPipeline::bind`.
	///
	/// [`FrameInFlight`]: crate::frame_in_flight::FrameInFlight
	unsafe fn to_transfer(self) -> Self::TransferDescStruct;

	/// On the GPU, transmute the transferable struct back to Self, keeping potential `'static` lifetimes.
	///
	/// # Safety
	/// Should only be implemented via DescBuffer macro and only used internally by `BufferSlice` functions.
	unsafe fn from_transfer(from: Self::TransferDescStruct, meta: Metadata) -> Self;
}

unsafe impl<T: DescStruct> DescBuffer for T {
	type TransferDescBuffer = T::TransferDescStruct;
}

// impl
unsafe impl<T: AnyBitPattern + Send + Sync> DescStruct for T {
	type TransferDescStruct = T;

	unsafe fn to_transfer(self) -> Self::TransferDescStruct {
		self
	}

	unsafe fn from_transfer(from: Self::TransferDescStruct, _meta: Metadata) -> Self {
		from
	}
}

unsafe impl<T: DescBuffer> DescBuffer for [T]
where
	T::TransferDescBuffer: Sized,
{
	type TransferDescBuffer = [T::TransferDescBuffer];
}
