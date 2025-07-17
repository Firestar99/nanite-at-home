use crate::descriptor::{DescContent, StrongDesc};
use bytemuck::AnyBitPattern;
use core::ops::Deref;

mod metadata;

pub use metadata::*;
pub use rust_gpu_bindless_buffer_content::*;

/// Trait for contents of **buffers** that may contain descriptors requiring conversion.
///
/// See [`BufferStruct`]. All [`BufferStruct`] also implement [`BufferContent`] with `TransferDescBuffer = TransferDescStruct`.
///
/// Compared to [`BufferStruct`], [`BufferContent`] also allows for unsized types such as slices to be used. Therefore, it
/// does not offer any conversion functions, since a slice can only be converted element-wise.
///
/// # Safety
/// Should not be manually implemented, see [`BufferStruct`].
pub unsafe trait BufferContent: Send + Sync {
	type Transfer: Send + Sync + ?Sized + 'static;
}

unsafe impl<T: BufferContent> BufferContent for [T]
where
	T::Transfer: Sized,
{
	type Transfer = [T::Transfer];
}

/// Trait for **sized types** that may contain descriptors requiring conversion and can be stored in a Buffer. Use
/// `#derive[DescBuffer]` on your type to implement this trait.
///
/// The actual type stored in the Buffer is defined by its associated type `TransferDescStruct` and can be converted to
/// and from using [`Self::to_transfer`] and [`Self::read`]. Types that are [`AnyBitPattern`] automatically
/// implement `DescBuffer` with conversions being identity.
///
/// # Safety
/// Should only be implemented via DescBuffer macro. Only Descriptors may have a manual implementation.
pub unsafe trait BufferStruct: Copy + Clone + Sized + Send + Sync {
	type Transfer: AnyBitPattern + Send + Sync;

	/// Transmute Self into a transferable struct on the CPU that can subsequently be sent to the GPU. This includes
	/// unsafely transmuting [`FrameInFlight`] lifetimes to `'static`, so it's [`AnyBitPattern`]`: 'static` and
	/// can be written to a buffer.
	///
	/// # Safety
	/// Should only be implemented via DescBuffer macro and only used internally by `BindlessPipeline::bind`.
	///
	/// [`FrameInFlight`]: crate::frame_in_flight::FrameInFlight
	unsafe fn write_cpu(self, meta: &mut impl MetadataCpuInterface) -> Self::Transfer;

	/// On the GPU, transmute the transferable struct back to Self, keeping potential `'static` lifetimes.
	///
	/// # Safety
	/// Should only be implemented via DescBuffer macro and only used internally by `BufferSlice` functions.
	unsafe fn read(from: Self::Transfer, meta: Metadata) -> Self;
}

unsafe impl<T: BufferStruct> BufferContent for T {
	type Transfer = T::Transfer;
}

/// An internal interface to CPU-only code. May change at any time.
///
/// # Safety
/// Internal interface to CPU code
pub unsafe trait MetadataCpuInterface: Deref<Target = Metadata> {
	fn visit_strong_descriptor<C: DescContent>(&mut self, desc: StrongDesc<C>);
}

unsafe impl<T: BufferStructPlain> BufferStruct for T {
	type Transfer = T::Transfer;

	unsafe fn write_cpu(self, _meta: &mut impl MetadataCpuInterface) -> Self::Transfer {
		unsafe { T::write(self) }
	}

	unsafe fn read(from: Self::Transfer, _meta: Metadata) -> Self {
		unsafe { T::read(from) }
	}
}
