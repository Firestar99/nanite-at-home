use crate::descriptor::boxed::{BoxDesc, BoxDescExt};
use crate::pipeline::access_type::{BufferAccessType, ShaderReadWriteable, ShaderReadable, Undefined};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{Buffer, MutBuffer, TransientDesc};
use std::marker::PhantomData;

pub struct MutBufferAccess<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> {
	desc: BoxDesc<P, MutBuffer<T>>,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized> MutBufferAccess<'a, P, T, Undefined> {
	pub unsafe fn new_dont_care(desc: BoxDesc<P, MutBuffer<T>>, cmd: &P::RecordingContext<'a>) -> Self {
		Self {
			desc,
			resource_context: cmd.resource_context(),
			_phantom: PhantomData,
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutBufferAccess<'a, P, T, A> {
	/// Transition this Buffer from one [`BufferAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: BufferAccessType>(self) -> MutBufferAccess<'a, P, T, B> {
		unsafe {
			self.resource_context
				.transition_buffer(&self.desc.inner_slot(), A::BUFFER_ACCESS, B::BUFFER_ACCESS);
		}
		MutBufferAccess {
			desc: self.desc,
			resource_context: self.resource_context,
			_phantom: PhantomData,
		}
	}

	pub fn into_inner(self) -> BoxDesc<P, MutBuffer<T>> {
		self.desc
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_transient(&self) -> TransientDesc<Buffer<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe { TransientDesc::new(self.desc.id(), &self.resource_context.to_transient_access()) }
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadWriteable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_mut_transient(&self) -> TransientDesc<MutBuffer<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadWriteable, so it is readable and writeable
		// by a shader
		unsafe { TransientDesc::new(self.desc.id(), &self.resource_context.to_transient_access()) }
	}
}
