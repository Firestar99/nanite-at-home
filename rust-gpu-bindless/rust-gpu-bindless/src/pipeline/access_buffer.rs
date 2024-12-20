use crate::backing::table::RcTableSlot;
use crate::descriptor::{BufferSlot, DescContentCpu, RCDesc, RCDescExt};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_lock::AccessLockError;
use crate::pipeline::access_type::{BufferAccess, BufferAccessType, ShaderReadWriteable, ShaderReadable};
use crate::platform::{BindlessPipelinePlatform, RecordingContext, RecordingResourceContext};
use rust_gpu_bindless_shaders::buffer_content::BufferContent;
use rust_gpu_bindless_shaders::descriptor::{Buffer, MutBuffer, TransientDesc};
use std::marker::PhantomData;

pub trait MutBufferAccessExt<P: BindlessPipelinePlatform, T: BufferContent + ?Sized>:
	MutDescExt<P, MutBuffer<T>>
{
	/// Access this mutable buffer to use it for recording.
	fn access<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessLockError>;

	/// Access this mutable buffer to use it for recording. Does not care for the contents of this buffer and assumes
	/// it is uninitialized.
	fn access_dont_care<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessLockError>;
}

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized> MutBufferAccessExt<P, T> for MutDesc<P, MutBuffer<T>> {
	fn access<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessLockError> {
		MutBufferAccess::from(self, cmd)
	}

	fn access_dont_care<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessLockError> {
		MutBufferAccess::from_dont_care(self, cmd)
	}
}

pub struct MutBufferAccess<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> {
	slot: RcTableSlot,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<T>,
	_phantom2: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutBufferAccess<'a, P, T, A> {
	pub fn from_dont_care(
		desc: MutDesc<P, MutBuffer<T>>,
		cmd: &P::RecordingContext<'a>,
	) -> Result<Self, AccessLockError> {
		Self::from_inner(desc, cmd, |_| BufferAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutBuffer<T>>, cmd: &P::RecordingContext<'a>) -> Result<Self, AccessLockError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutBuffer<T>>,
		cmd: &P::RecordingContext<'a>,
		f: impl FnOnce(BufferAccess) -> BufferAccess,
	) -> Result<Self, AccessLockError> {
		let this = Self {
			slot: desc.into_rc_slot(),
			resource_context: cmd.resource_context(),
			_phantom: PhantomData,
			_phantom2: PhantomData,
		};
		this.transition_inner(f(this.inner_slot().access_lock.try_lock()?), A::BUFFER_ACCESS);
		Ok(this)
	}

	/// Transition this Buffer from one [`BufferAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: BufferAccessType>(self) -> MutBufferAccess<'a, P, T, B> {
		self.transition_inner(A::BUFFER_ACCESS, B::BUFFER_ACCESS);
		MutBufferAccess {
			slot: self.slot,
			resource_context: self.resource_context,
			_phantom: PhantomData,
			_phantom2: PhantomData,
		}
	}

	#[inline]
	fn transition_inner(&self, src: BufferAccess, dst: BufferAccess) {
		// TODO check buffer usage if transition is valid
		if src != dst {
			unsafe { self.resource_context.transition_buffer(&self.inner_slot(), src, dst) }
		}
	}

	#[inline]
	fn inner_slot(&self) -> &BufferSlot<P> {
		MutBuffer::<T>::get_slot(&self.slot)
	}

	// TODO these technically unlock the slot too early, one would have to wait until the execution finished to unlock
	//  them, as otherwise two executions may race on this resource. When impl, also add an unsafe variant for instant
	//  unlock, which is useful for frame in flight shared resources.
	/// Turns this mutable access to a [`MutBuffer`] back into a [`MutBuffer`] to be used in another execution
	pub fn into_desc(self) -> MutDesc<P, MutBuffer<T>> {
		self.inner_slot().access_lock.unlock(A::BUFFER_ACCESS);
		unsafe { MutDesc::new(self.slot) }
	}

	/// Turns this mutable access to a [`MutBuffer`] into a shared [`RCDesc`]
	pub fn into_shared(self) -> RCDesc<P, Buffer<T>> {
		unsafe {
			self.transition_inner(A::BUFFER_ACCESS, BufferAccess::GeneralRead);
			self.inner_slot().access_lock.unlock_to_shared();
			RCDesc::new(self.slot)
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_transient(&self) -> TransientDesc<Buffer<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe { TransientDesc::new(self.slot.id(), &self.resource_context.to_transient_access()) }
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadWriteable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_mut_transient(&self) -> TransientDesc<MutBuffer<T>> {
		// Safety: mutable resource is in a layout that implements ShaderReadWriteable, so it is readable and writeable
		// by a shader
		unsafe { TransientDesc::new(self.slot.id(), &self.resource_context.to_transient_access()) }
	}
}
