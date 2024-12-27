use crate::backing::table::RcTableSlot;
use crate::descriptor::{BindlessBufferUsage, BufferSlot, BufferTable, DescTable, RCDesc, RCDescExt};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_error::AccessError;
use crate::pipeline::access_type::{BufferAccess, BufferAccessType, ShaderReadWriteable, ShaderReadable};
use crate::pipeline::mut_or_shared::MutOrSharedBuffer;
use crate::pipeline::recording::HasResourceContext;
use crate::platform::{BindlessPipelinePlatform, RecordingResourceContext};
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
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError>;

	/// Access this mutable buffer to use it for recording. Discards the contents of this buffer and as if it were
	/// uninitialized.
	///
	/// # Safety
	/// Must not read uninitialized memory and fully overwrite it within this execution context.
	unsafe fn access_undefined_contents<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError>;
}

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized> MutBufferAccessExt<P, T> for MutDesc<P, MutBuffer<T>> {
	fn access<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError> {
		MutBufferAccess::from(self, cmd)
	}

	unsafe fn access_undefined_contents<'a, A: BufferAccessType>(
		self,
		cmd: &P::RecordingContext<'a>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError> {
		MutBufferAccess::from_undefined_contents(self, cmd)
	}
}

pub struct MutBufferAccess<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> {
	slot: RcTableSlot,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<T>,
	_phantom2: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutBufferAccess<'a, P, T, A> {
	pub fn from_undefined_contents(
		desc: MutDesc<P, MutBuffer<T>>,
		cmd: &P::RecordingContext<'a>,
	) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |_| BufferAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutBuffer<T>>, cmd: &P::RecordingContext<'a>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutBuffer<T>>,
		cmd: &P::RecordingContext<'a>,
		f: impl FnOnce(BufferAccess) -> BufferAccess,
	) -> Result<Self, AccessError> {
		unsafe {
			let this = Self {
				slot: desc.into_rc_slot(),
				resource_context: cmd.resource_context(),
				_phantom: PhantomData,
				_phantom2: PhantomData,
			};
			this.transition_inner(f(this.inner_slot().access_lock.try_lock()?), A::BUFFER_ACCESS)?;
			Ok(this)
		}
	}

	/// Transition this Buffer from one [`BufferAccessType`] to another and inserts appropriate barriers.
	pub fn transition<B: BufferAccessType>(self) -> Result<MutBufferAccess<'a, P, T, B>, AccessError> {
		self.transition_inner(A::BUFFER_ACCESS, B::BUFFER_ACCESS)?;
		Ok(MutBufferAccess {
			slot: self.slot,
			resource_context: self.resource_context,
			_phantom: PhantomData,
			_phantom2: PhantomData,
		})
	}

	#[inline]
	fn transition_inner(&self, src: BufferAccess, dst: BufferAccess) -> Result<(), AccessError> {
		unsafe {
			self.has_required_usage(dst.required_buffer_usage())?;
			if src != dst {
				self.resource_context.transition_buffer(self.inner_slot(), src, dst)
			}
			Ok(())
		}
	}

	#[inline]
	pub unsafe fn inner_slot(&self) -> &BufferSlot<P> {
		BufferTable::get_slot(&self.slot)
	}

	// TODO these technically unlock the slot too early, one would have to wait until the execution finished to unlock
	//  them, as otherwise two executions may race on this resource. When impl, also add an unsafe variant for instant
	//  unlock, which is useful for frame in flight shared resources.
	/// Turns this mutable access to a [`MutBuffer`] back into a [`MutBuffer`] to be used in another execution
	pub fn into_desc(self) -> MutDesc<P, MutBuffer<T>> {
		unsafe {
			self.inner_slot().access_lock.unlock(A::BUFFER_ACCESS);
			MutDesc::new(self.slot)
		}
	}

	/// Turns this mutable access to a [`MutBuffer`] into a shared [`RCDesc`]
	pub fn into_shared(self) -> RCDesc<P, Buffer<T>> {
		unsafe {
			// cannot fail
			self.transition_inner(A::BUFFER_ACCESS, BufferAccess::GeneralRead)
				.unwrap();
			self.inner_slot().access_lock.unlock_to_shared();
			RCDesc::new(self.slot)
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_transient(&self) -> Result<TransientDesc<Buffer<T>>, AccessError> {
		self.has_required_usage(BindlessBufferUsage::STORAGE_BUFFER)?;
		// Safety: mutable resource is in a layout that implements ShaderReadable, so it is readable by a shader
		unsafe {
			Ok(TransientDesc::new(
				self.slot.id(),
				&self.resource_context.to_transient_access(),
			))
		}
	}
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadWriteable>
	MutBufferAccess<'a, P, T, A>
{
	pub fn to_mut_transient(&self) -> Result<TransientDesc<MutBuffer<T>>, AccessError> {
		self.has_required_usage(BindlessBufferUsage::STORAGE_BUFFER)?;
		// Safety: mutable resource is in a layout that implements ShaderReadWriteable, so it is readable and writeable
		// by a shader
		unsafe {
			Ok(TransientDesc::new(
				self.slot.id(),
				&self.resource_context.to_transient_access(),
			))
		}
	}
}
