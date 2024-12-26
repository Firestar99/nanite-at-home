use crate::descriptor::{BufferSlot, ImageSlot, RCDesc, RCDescExt};
use crate::pipeline::access_buffer::MutBufferAccess;
use crate::pipeline::access_image::MutImageAccess;
use crate::pipeline::access_type::{BufferAccessType, ImageAccessType};
use crate::platform::{BindlessPipelinePlatform, BindlessPlatform};
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{Buffer, Image, ImageType};

pub unsafe trait MutOrSharedBuffer<P: BindlessPlatform, T: BufferContent + ?Sized, A> {
	unsafe fn inner_slot(&self) -> &BufferSlot<P>;
}

unsafe impl<P: BindlessPlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutOrSharedBuffer<P, T, A>
	for RCDesc<P, Buffer<T>>
{
	unsafe fn inner_slot(&self) -> &BufferSlot<P> {
		RCDescExt::inner_slot(self)
	}
}

unsafe impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutOrSharedBuffer<P, T, A>
	for MutBufferAccess<'_, P, T, A>
{
	unsafe fn inner_slot(&self) -> &BufferSlot<P> {
		unsafe { MutBufferAccess::inner_slot(self) }
	}
}

pub unsafe trait MutOrSharedImage<P: BindlessPlatform, T: ImageType, A> {
	unsafe fn inner_slot(&self) -> &ImageSlot<P>;
}

unsafe impl<P: BindlessPlatform, T: ImageType, A: ImageAccessType> MutOrSharedImage<P, T, A> for RCDesc<P, Image<T>> {
	unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		RCDescExt::inner_slot(self)
	}
}

unsafe impl<P: BindlessPipelinePlatform, T: ImageType, A: ImageAccessType> MutOrSharedImage<P, T, A>
	for MutImageAccess<'_, P, T, A>
{
	unsafe fn inner_slot(&self) -> &ImageSlot<P> {
		unsafe { MutImageAccess::inner_slot(self) }
	}
}
