use crate::backing::table::RcTableSlot;
use crate::descriptor::{BindlessBufferUsage, BufferSlot, BufferTable, DescTable, RCDesc, RCDescExt};
use crate::descriptor::{MutDesc, MutDescExt};
use crate::pipeline::access_error::AccessError;
use crate::pipeline::access_type::{BufferAccess, BufferAccessType, ShaderReadWriteable, ShaderReadable};
use crate::pipeline::mut_or_shared::MutOrSharedBuffer;
use crate::pipeline::recording::{HasResourceContext, Recording};
use crate::platform::{BindlessPipelinePlatform, RecordingResourceContext};
use bytemuck::Pod;
use rust_gpu_bindless_shaders::buffer_content::{BufferContent, BufferStruct, BufferStructPlain};
use rust_gpu_bindless_shaders::descriptor::{Buffer, MutBuffer, TransientDesc};
use std::future::Future;
use std::marker::PhantomData;

pub trait MutBufferAccessExt<P: BindlessPipelinePlatform, T: BufferContent + ?Sized>:
	MutDescExt<P, MutBuffer<T>>
{
	/// Access this mutable buffer to use it for recording.
	fn access<'a, A: BufferAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError>;

	/// Access this mutable buffer to use it for recording. Discards the contents of this buffer and acts as if it were
	/// uninitialized.
	///
	/// # Safety
	/// Must not read uninitialized memory and must fully overwrite it within this execution context.
	unsafe fn access_as_undefined<'a, A: BufferAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError>;
}

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized> MutBufferAccessExt<P, T> for MutDesc<P, MutBuffer<T>> {
	fn access<'a, A: BufferAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError> {
		MutBufferAccess::from(self, cmd)
	}

	unsafe fn access_as_undefined<'a, A: BufferAccessType>(
		self,
		cmd: &Recording<'a, P>,
	) -> Result<MutBufferAccess<'a, P, T, A>, AccessError> {
		unsafe { MutBufferAccess::from_undefined(self, cmd) }
	}
}

pub struct MutBufferAccess<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> {
	slot: RcTableSlot,
	resource_context: &'a P::RecordingResourceContext,
	_phantom: PhantomData<T>,
	_phantom2: PhantomData<A>,
}

impl<'a, P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType> MutBufferAccess<'a, P, T, A> {
	/// See [`MutBufferAccessExt::access_as_undefined`]
	///
	/// # Safety
	/// Must not read uninitialized memory and must fully overwrite it within this execution context.
	pub unsafe fn from_undefined(desc: MutDesc<P, MutBuffer<T>>, cmd: &Recording<'a, P>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |_| BufferAccess::Undefined)
	}

	pub fn from(desc: MutDesc<P, MutBuffer<T>>, cmd: &Recording<'a, P>) -> Result<Self, AccessError> {
		Self::from_inner(desc, cmd, |x| x)
	}

	#[inline]
	fn from_inner(
		desc: MutDesc<P, MutBuffer<T>>,
		cmd: &Recording<'a, P>,
		f: impl FnOnce(BufferAccess) -> BufferAccess,
	) -> Result<Self, AccessError> {
		unsafe {
			let (slot, last) = desc.into_inner();
			cmd.resource_context().add_dependency(last);
			let this = Self {
				slot,
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
			MutDesc::new(self.slot, self.resource_context.to_pending_execution())
		}
	}

	/// Turns this mutable access to a [`MutBuffer`] into a shared [`RCDesc`]
	pub fn into_shared(self) -> impl Future<Output = RCDesc<P, Buffer<T>>> + use<P, T, A> {
		unsafe {
			// cannot fail
			self.transition_inner(A::BUFFER_ACCESS, BufferAccess::GeneralRead)
				.unwrap();
			let pending_execution = self.resource_context.to_pending_execution();
			let slot = self.slot;
			async move {
				pending_execution.await;
				BufferTable::<P>::get_slot(&slot).access_lock.unlock_to_shared();
				RCDesc::new(slot)
			}
		}
	}
}

impl<P: BindlessPipelinePlatform, T: BufferStruct, A: BufferAccessType> MutBufferAccess<'_, P, [T], A> {
	pub fn len(&self) -> usize {
		unsafe { self.inner_slot().len }
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl<P: BindlessPipelinePlatform, T: BufferStructPlain, A: BufferAccessType, const N: usize>
	MutBufferAccess<'_, P, [T; N], A>
where
	// see `impl BufferStructPlain for [T; N]`
	T: Default,
	T::Transfer: Pod + Default,
{
	pub const fn len(&self) -> usize {
		N
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadable>
	MutBufferAccess<'_, P, T, A>
{
	pub fn to_transient(&self) -> Result<TransientDesc<'_, Buffer<T>>, AccessError> {
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

impl<P: BindlessPipelinePlatform, T: BufferContent + ?Sized, A: BufferAccessType + ShaderReadWriteable>
	MutBufferAccess<'_, P, T, A>
{
	pub fn to_mut_transient(&self) -> Result<TransientDesc<'_, MutBuffer<T>>, AccessError> {
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
