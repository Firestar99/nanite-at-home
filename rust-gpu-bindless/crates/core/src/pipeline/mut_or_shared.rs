use crate::descriptor::{BindlessBufferUsage, BindlessImageUsage, BufferSlot, ImageSlot, RCDesc, RCDescExt};
use crate::pipeline::{AccessError, BufferAccessType, GeneralRead, ImageAccessType, MutBufferAccess, MutImageAccess};
use crate::platform::{BindlessPipelinePlatform, BindlessPlatform};
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{Buffer, Image, ImageType};

/// A read-only buffer that is either a [`MutBufferAccess`] in [`GeneralRead`] layout or a shared read-only [`RCDesc`]
/// buffer.
///
/// # Safety
/// Must reference a read-only buffer
pub unsafe trait MutOrSharedBuffer<P: BindlessPlatform, T: BufferContent + ?Sized, A> {
	unsafe fn inner_slot(&self) -> &BufferSlot<P>;

	/// Verify that this buffer has all the usages given by param.
	#[inline]
	fn has_required_usage(&self, required: BindlessBufferUsage) -> Result<(), AccessError> {
		unsafe {
			let slot = self.inner_slot();
			if !slot.usage.contains(required) {
				Err(AccessError::MissingBufferUsage {
					name: slot.debug_name().to_string(),
					usage: slot.usage,
					missing_usage: required,
				})
			} else {
				Ok(())
			}
		}
	}
}

unsafe impl<P: BindlessPlatform, T: BufferContent + ?Sized> MutOrSharedBuffer<P, T, GeneralRead>
	for &RCDesc<P, Buffer<T>>
{
	unsafe fn inner_slot(&self) -> &BufferSlot<P> {
		RCDescExt::inner_slot(*self)
	}
}

unsafe impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutOrSharedBuffer<P, T, A>
	for &MutBufferAccess<'_, P, T, A>
{
	unsafe fn inner_slot(&self) -> &BufferSlot<P> {
		unsafe { MutBufferAccess::inner_slot(self) }
	}
}

/// A read-only image that is either a [`MutImageAccess`] in [`GeneralRead`] layout or a shared read-only [`RCDesc`]
/// image.
///
/// # Safety
/// Must reference a read-only buffer
pub unsafe trait MutOrSharedImage<P: BindlessPlatform, T: ImageType, A> {
	unsafe fn inner_slot(&self) -> &ImageSlot<P>;

	/// Verify that this image has all the usages given by param.
	#[inline]
	fn has_required_usage(&self, required: BindlessImageUsage) -> Result<(), AccessError> {
		unsafe {
			let slot = self.inner_slot();
			if !slot.usage.contains(required) {
				Err(AccessError::MissingImageUsage {
					name: slot.debug_name().to_string(),
					usage: slot.usage,
					missing_usage: required,
				})
			} else {
				Ok(())
			}
		}
	}
}

unsafe impl<P: BindlessPlatform, T: ImageType> MutOrSharedImage<P, T, GeneralRead> for &RCDesc<P, Image<T>> {
	unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		RCDescExt::inner_slot(*self)
	}
}

unsafe impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> MutOrSharedImage<P, T, A>
	for &MutImageAccess<'_, P, T, A>
{
	unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		unsafe { MutImageAccess::inner_slot(self) }
	}
}
